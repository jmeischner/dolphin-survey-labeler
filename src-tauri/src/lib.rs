use csv::WriterBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use walkdir::WalkDir;

const DEFAULT_RULES_JSON: &str = include_str!("../assets/rules.default.json");

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
    #[error(transparent)]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rules {
    pub extensions: Vec<String>,
    pub survey_id_regex_detected: String,
    pub survey_id_regex_base: String,
    #[serde(default = "default_image_id_regex")]
    pub image_id_regex: String,
    pub graded_priority_ind_regex: String,
    pub graded_priority_secondary_tokens: Vec<String>,
    pub graded_negative_contains_any: Vec<String>,
    pub graded_positive_contains_any: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootRunOptions {
    pub write_per_survey: bool,
    pub write_merged: bool,
    pub merged_filename: String,
    pub problems_filename: String,
    pub per_survey_dirname: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleRunOptions {
    pub output_filename: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewItem {
    pub base_key: String,
    pub raw_path: Option<String>,
    pub graded_path: Option<String>,
    pub status: String,
    pub problem_type: Option<String>,
    pub details: Option<String>,
    pub raw_image_count: Option<u64>,
    pub graded_image_count: Option<u64>,
    pub survey_id_raw_detected: Option<String>,
    pub survey_id_graded_detected: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub survey_id_base: String,
    pub processed: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub processed_surveys: u64,
    pub total_rows: u64,
    pub dolphin_yes: u64,
    pub dolphin_no: u64,
    pub ambiguity_warnings: u64,
    pub problems_count: u64,
    pub output_dir: String,
    pub merged_csv_path: Option<String>,
    pub problems_csv_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemItem {
    pub survey_id_base: String,
    pub survey_id_detected: Option<String>,
    pub raw_path: Option<String>,
    pub graded_path: Option<String>,
    pub problem_type: String,
    pub details: Option<String>,
}

#[derive(Clone, Debug)]
struct SurveyFolder {
    path: PathBuf,
    detected_id: Option<String>,
}

#[derive(Clone, Debug)]
struct CandidateWinner {
    relpath: String,
    winner_type: String,
}

#[derive(Clone, Debug)]
struct PairResult {
    rows: Vec<CsvRow>,
    ambiguity_warnings: u64,
}

#[derive(Clone, Debug)]
struct GradedMapResult {
    map: HashMap<String, Vec<String>>,
    ambiguity_warnings: u64,
}

#[derive(Clone, Debug)]
struct CompiledRules {
    extensions: HashSet<String>,
    detected_re: Regex,
    base_re: Regex,
    image_id_re: Regex,
    ind_re: Regex,
    secondary_tokens: Vec<String>,
    negative_tokens: Vec<String>,
    positive_tokens: Vec<String>,
}

#[derive(Clone, Debug)]
struct CsvRow {
    survey_id_base: String,
    raw_relpath: String,
    filename: String,
    dolphin: u8,
    graded_relpath: String,
    graded_hits: u64,
    graded_winner_type: String,
    survey_id_raw_detected: Option<String>,
    survey_id_graded_detected: Option<String>,
}

pub fn get_or_init_rules(app: &AppHandle) -> Result<Rules, AppError> {
    let path = rules_file_path(app)?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, DEFAULT_RULES_JSON)?;
    }
    let data = fs::read_to_string(path)?;
    let rules: Rules = serde_json::from_str(&data)?;
    Ok(rules)
}

pub fn save_rules(app: &AppHandle, rules: Rules) -> Result<Rules, AppError> {
    let path = rules_file_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(&rules)?;
    fs::write(&path, data)?;
    Ok(rules)
}

pub fn reset_rules(app: &AppHandle) -> Result<Rules, AppError> {
    let rules: Rules = serde_json::from_str(DEFAULT_RULES_JSON)?;
    save_rules(app, rules)
}

pub fn preview_root_scan(
    graded_root: PathBuf,
    raw_root: PathBuf,
    rules: Rules,
) -> Result<Vec<PreviewItem>, AppError> {
    let compiled = compile_rules(&rules)?;
    let scan = scan_roots(&raw_root, &graded_root, &compiled, true)?;
    Ok(scan.preview)
}

pub fn run_root_scan(
    app: &AppHandle,
    graded_root: PathBuf,
    raw_root: PathBuf,
    output_dir: PathBuf,
    options: RootRunOptions,
    rules: Rules,
) -> Result<RunSummary, AppError> {
    let compiled = compile_rules(&rules)?;
    let scan = scan_roots(&raw_root, &graded_root, &compiled, false)?;

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
    }

    let per_survey_dir = output_dir.join(&options.per_survey_dirname);
    if options.write_per_survey {
        fs::create_dir_all(&per_survey_dir)?;
    }

    let problems_csv_path = output_dir.join(&options.problems_filename);
    if !scan.problems.is_empty() {
        write_problems_csv(&problems_csv_path, &scan.problems)?;
    }

    let mut merged_writer = if options.write_merged {
        let path = output_dir.join(&options.merged_filename);
        Some(init_csv_writer(&path)?)
    } else {
        None
    };

    let mut processed_surveys = 0u64;
    let mut total_rows = 0u64;
    let mut dolphin_yes = 0u64;
    let mut dolphin_no = 0u64;
    let mut ambiguity_warnings = 0u64;

    for entry in scan.entries {
        if entry.status != "OK" {
            continue;
        }
        let raw = entry.raw.expect("raw required");
        let graded = entry.graded.expect("graded required");

        let pair_result = process_pair(app, &compiled, &entry.base_key, &raw, &graded)?;
        let rows = pair_result.rows;
        ambiguity_warnings += pair_result.ambiguity_warnings;

        if options.write_per_survey {
            let per_path = per_survey_dir.join(format!("{}.csv", entry.base_key));
            write_csv_rows(&per_path, &rows)?;
        }

        if let Some(writer) = merged_writer.as_mut() {
            write_rows_to_writer(writer, &rows)?;
        }

        processed_surveys += 1;
        for row in rows {
            total_rows += 1;
            if row.dolphin == 1 {
                dolphin_yes += 1;
            } else {
                dolphin_no += 1;
            }
        }
    }

    let merged_csv_path = if options.write_merged {
        Some(
            output_dir
                .join(&options.merged_filename)
                .to_string_lossy()
                .to_string(),
        )
    } else {
        None
    };

    let problems_csv_path = if !scan.problems.is_empty() {
        Some(problems_csv_path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(RunSummary {
        processed_surveys,
        total_rows,
        dolphin_yes,
        dolphin_no,
        ambiguity_warnings,
        problems_count: scan.problems.len() as u64,
        output_dir: output_dir.to_string_lossy().to_string(),
        merged_csv_path,
        problems_csv_path,
    })
}

pub fn run_single_pair(
    app: &AppHandle,
    graded_dir: PathBuf,
    raw_dir: PathBuf,
    output_dir: PathBuf,
    survey_id_override: Option<String>,
    options: SingleRunOptions,
    rules: Rules,
) -> Result<RunSummary, AppError> {
    let compiled = compile_rules(&rules)?;
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
    }

    let detected = survey_id_override
        .and_then(|value| extract_base_key(&value, &compiled.base_re).map(|base| (value, base)))
        .or_else(|| {
            extract_detected_id(&graded_dir, &compiled.detected_re).and_then(|detected| {
                extract_base_key(&detected, &compiled.base_re).map(|base| (detected, base))
            })
        })
        .ok_or_else(|| {
            AppError::Message(
                "Unable to derive survey id base; please provide an override.".to_string(),
            )
        })?;

    let (detected_full, base_key) = detected;

    let raw_detected = extract_detected_id(&raw_dir, &compiled.detected_re);
    let raw_folder = SurveyFolder {
        path: raw_dir,
        detected_id: raw_detected,
    };
    let graded_folder = SurveyFolder {
        path: graded_dir,
        detected_id: Some(detected_full.clone()),
    };

    let pair_result = process_pair(app, &compiled, &base_key, &raw_folder, &graded_folder)?;
    let rows = pair_result.rows;
    let output_path = output_dir.join(&options.output_filename);
    write_csv_rows(&output_path, &rows)?;

    let mut dolphin_yes = 0u64;
    let mut dolphin_no = 0u64;
    for row in &rows {
        if row.dolphin == 1 {
            dolphin_yes += 1;
        } else {
            dolphin_no += 1;
        }
    }

    Ok(RunSummary {
        processed_surveys: 1,
        total_rows: rows.len() as u64,
        dolphin_yes,
        dolphin_no,
        ambiguity_warnings: pair_result.ambiguity_warnings,
        problems_count: 0,
        output_dir: output_dir.to_string_lossy().to_string(),
        merged_csv_path: Some(output_path.to_string_lossy().to_string()),
        problems_csv_path: None,
    })
}

fn rules_file_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AppError::Message(err.to_string()))?;
    Ok(dir.join("rules.json"))
}

fn default_image_id_regex() -> String {
    "^(.+?_\\d{3,5})(?:[ _][A-Za-z0-9]+)*$".to_string()
}

fn compile_rules(rules: &Rules) -> Result<CompiledRules, AppError> {
    let mut extensions = HashSet::new();
    for ext in &rules.extensions {
        let normalized = normalize_extension(ext);
        extensions.insert(normalized);
    }
    Ok(CompiledRules {
        extensions,
        detected_re: Regex::new(&rules.survey_id_regex_detected)?,
        base_re: Regex::new(&rules.survey_id_regex_base)?,
        image_id_re: Regex::new(&rules.image_id_regex)?,
        ind_re: Regex::new(&rules.graded_priority_ind_regex)?,
        secondary_tokens: normalize_tokens(&rules.graded_priority_secondary_tokens),
        negative_tokens: normalize_tokens(&rules.graded_negative_contains_any),
        positive_tokens: normalize_tokens(&rules.graded_positive_contains_any),
    })
}

fn normalize_extension(ext: &str) -> String {
    let trimmed = ext.trim().to_lowercase();
    if trimmed.starts_with('.') {
        trimmed
    } else {
        format!(".{}", trimmed)
    }
}

fn normalize_tokens(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn extract_detected_id(path: &Path, regex: &Regex) -> Option<String> {
    let path_str = path.to_string_lossy();
    regex
        .captures_iter(&path_str)
        .last()
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_base_key(value: &str, regex: &Regex) -> Option<String> {
    regex
        .captures_iter(value)
        .last()
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_uppercase())
}

fn scan_roots(
    raw_root: &Path,
    graded_root: &Path,
    rules: &CompiledRules,
    include_counts: bool,
) -> Result<ScanResult, AppError> {
    let raw_map = discover_surveys(raw_root, rules)?;
    let graded_map = discover_surveys(graded_root, rules)?;

    let mut base_keys: HashSet<String> = raw_map.keys().cloned().collect();
    base_keys.extend(graded_map.keys().cloned());

    let mut entries = Vec::new();
    let mut problems = Vec::new();
    let mut preview = Vec::new();

    for base_key in base_keys {
        let raw_list = raw_map.get(&base_key).cloned().unwrap_or_default();
        let graded_list = graded_map.get(&base_key).cloned().unwrap_or_default();

        let raw_missing = raw_list.is_empty();
        let graded_missing = graded_list.is_empty();

        let (raw, raw_problem) = select_unique(&base_key, &raw_list, "DUPLICATE_RAW");
        let (graded, graded_problem) = select_unique(&base_key, &graded_list, "DUPLICATE_GRADED");

        if let Some(problem) = raw_problem.as_ref() {
            problems.push(problem.clone());
        }
        if let Some(problem) = graded_problem.as_ref() {
            problems.push(problem.clone());
        }

        let mut status = "OK".to_string();
        let mut problem_type = None;
        let mut details = None;

        if raw_missing {
            status = "PROBLEM".to_string();
            problem_type = Some("RAW_MISSING".to_string());
            details = Some("No raw survey folder found.".to_string());
            problems.push(ProblemItem {
                survey_id_base: base_key.clone(),
                survey_id_detected: graded
                    .as_ref()
                    .and_then(|folder| folder.detected_id.clone()),
                raw_path: None,
                graded_path: graded
                    .as_ref()
                    .map(|folder| folder.path.to_string_lossy().to_string()),
                problem_type: "RAW_MISSING".to_string(),
                details: None,
            });
        }

        if graded_missing {
            status = "PROBLEM".to_string();
            problem_type = Some("GRADED_MISSING".to_string());
            details = Some("No graded survey folder found.".to_string());
            problems.push(ProblemItem {
                survey_id_base: base_key.clone(),
                survey_id_detected: raw.as_ref().and_then(|folder| folder.detected_id.clone()),
                raw_path: raw
                    .as_ref()
                    .map(|folder| folder.path.to_string_lossy().to_string()),
                graded_path: None,
                problem_type: "GRADED_MISSING".to_string(),
                details: None,
            });
        }

        if raw_problem.is_some() || graded_problem.is_some() {
            status = "PROBLEM".to_string();
            if problem_type.is_none() {
                problem_type = raw_problem
                    .as_ref()
                    .map(|problem| problem.problem_type.clone())
                    .or_else(|| {
                        graded_problem
                            .as_ref()
                            .map(|problem| problem.problem_type.clone())
                    });
                details = raw_problem
                    .as_ref()
                    .and_then(|problem| problem.details.clone())
                    .or_else(|| {
                        graded_problem
                            .as_ref()
                            .and_then(|problem| problem.details.clone())
                    });
            }
        }

        let (raw_count, graded_count) = if include_counts {
            let raw_count = raw
                .as_ref()
                .map(|folder| count_images(&folder.path, rules))
                .transpose()?;
            let graded_count = graded
                .as_ref()
                .map(|folder| count_images(&folder.path, rules))
                .transpose()?;
            (raw_count, graded_count)
        } else {
            (None, None)
        };

        let preview_item = PreviewItem {
            base_key: base_key.clone(),
            raw_path: raw
                .as_ref()
                .map(|folder| folder.path.to_string_lossy().to_string()),
            graded_path: graded
                .as_ref()
                .map(|folder| folder.path.to_string_lossy().to_string()),
            status: status.clone(),
            problem_type: problem_type.clone(),
            details: details.clone(),
            raw_image_count: raw_count,
            graded_image_count: graded_count,
            survey_id_raw_detected: raw.as_ref().and_then(|folder| folder.detected_id.clone()),
            survey_id_graded_detected: graded
                .as_ref()
                .and_then(|folder| folder.detected_id.clone()),
        };

        preview.push(preview_item);
        entries.push(ScanEntry {
            base_key,
            raw,
            graded,
            status,
        });
    }

    preview.sort_by(|a, b| a.base_key.cmp(&b.base_key));
    entries.sort_by(|a, b| a.base_key.cmp(&b.base_key));

    Ok(ScanResult {
        entries,
        problems,
        preview,
    })
}

fn select_unique(
    base_key: &str,
    list: &[SurveyFolder],
    problem_type: &str,
) -> (Option<SurveyFolder>, Option<ProblemItem>) {
    if list.len() <= 1 {
        return (list.first().cloned(), None);
    }
    let detail = list
        .iter()
        .map(|item| item.path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("; ");
    (
        None,
        Some(ProblemItem {
            survey_id_base: base_key.to_string(),
            survey_id_detected: list.first().and_then(|folder| folder.detected_id.clone()),
            raw_path: None,
            graded_path: None,
            problem_type: problem_type.to_string(),
            details: Some(detail),
        }),
    )
}

fn discover_surveys(
    root: &Path,
    rules: &CompiledRules,
) -> Result<HashMap<String, Vec<SurveyFolder>>, AppError> {
    let mut map: HashMap<String, Vec<SurveyFolder>> = HashMap::new();
    let mut walker = WalkDir::new(root).into_iter();
    while let Some(entry) = walker.next() {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let detected_id = extract_detected_id(path, &rules.detected_re);
        let base_key = detected_id
            .as_ref()
            .and_then(|detected| extract_base_key(detected, &rules.base_re))
            .or_else(|| {
                let path_str = path.to_string_lossy();
                extract_base_key(&path_str, &rules.base_re)
            });
        if let Some(base_key) = base_key {
            map.entry(base_key).or_default().push(SurveyFolder {
                path: path.to_path_buf(),
                detected_id,
            });
            walker.skip_current_dir();
        }
    }
    Ok(map)
}

fn count_images(root: &Path, rules: &CompiledRules) -> Result<u64, AppError> {
    let mut count = 0u64;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if is_supported_image(entry.path(), rules) {
            count += 1;
        }
    }
    Ok(count)
}

fn is_supported_image(path: &Path, rules: &CompiledRules) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            rules
                .extensions
                .contains(&format!(".{}", ext.to_lowercase()))
        })
        .unwrap_or(false)
}

fn process_pair(
    app: &AppHandle,
    rules: &CompiledRules,
    base_key: &str,
    raw: &SurveyFolder,
    graded: &SurveyFolder,
) -> Result<PairResult, AppError> {
    let graded_result = build_graded_map(&graded.path, rules)?;
    let graded_map = graded_result.map;
    let raw_files = collect_images(&raw.path, rules)?;
    let total = raw_files.len() as u64;

    let mut rows = Vec::new();
    let mut ambiguity_warnings = graded_result.ambiguity_warnings;
    for (index, raw_path) in raw_files.into_iter().enumerate() {
        let (file_id, ambiguous) = compute_file_id(&raw_path, rules);
        if ambiguous {
            ambiguity_warnings += 1;
        }
        let candidates = graded_map.get(&file_id).cloned().unwrap_or_default();
        let winner = select_winner(&candidates, rules);
        let (dolphin, graded_relpath, winner_type) = if candidates.is_empty() {
            (0u8, "RAW".to_string(), "RAW".to_string())
        } else {
            let has_negative = any_token_match(&candidates, &rules.negative_tokens);
            let positive_ok = if rules.positive_tokens.is_empty()
                || rules.positive_tokens.iter().any(|token| token == "*")
            {
                true
            } else {
                any_token_match(&candidates, &rules.positive_tokens)
            };
            let dolphin = if !has_negative && positive_ok {
                1u8
            } else {
                0u8
            };
            (
                dolphin,
                winner
                    .as_ref()
                    .map(|value| value.relpath.clone())
                    .unwrap_or_else(|| "RAW".to_string()),
                winner
                    .as_ref()
                    .map(|value| value.winner_type.clone())
                    .unwrap_or_else(|| "RAW".to_string()),
            )
        };

        let raw_relpath = normalize_relpath(&raw_path, &raw.path);
        let filename = raw_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        rows.push(CsvRow {
            survey_id_base: base_key.to_string(),
            raw_relpath,
            filename,
            dolphin,
            graded_relpath,
            graded_hits: candidates.len() as u64,
            graded_winner_type: winner_type,
            survey_id_raw_detected: raw.detected_id.clone(),
            survey_id_graded_detected: graded.detected_id.clone(),
        });

        let _ = app.emit(
            "progress",
            ProgressEvent {
                survey_id_base: base_key.to_string(),
                processed: (index as u64) + 1,
                total,
            },
        );
    }

    Ok(PairResult {
        rows,
        ambiguity_warnings,
    })
}

fn collect_images(root: &Path, rules: &CompiledRules) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if is_supported_image(entry.path(), rules) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

fn build_graded_map(
    graded_root: &Path,
    rules: &CompiledRules,
) -> Result<GradedMapResult, AppError> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut ambiguity_warnings = 0u64;
    for entry in WalkDir::new(graded_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if !is_supported_image(entry.path(), rules) {
            continue;
        }
        let (file_id, ambiguous) = compute_file_id(entry.path(), rules);
        if ambiguous {
            ambiguity_warnings += 1;
        }
        let relpath = normalize_relpath(entry.path(), graded_root);
        map.entry(file_id).or_default().push(relpath);
    }
    Ok(GradedMapResult {
        map,
        ambiguity_warnings,
    })
}

fn compute_file_id(path: &Path, rules: &CompiledRules) -> (String, bool) {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if let Some(captures) = rules.image_id_re.captures(stem) {
        if let Some(matched) = captures.get(1) {
            return (matched.as_str().to_lowercase(), false);
        }
    }
    let filename_lower = filename.to_lowercase();
    match fs::metadata(path) {
        Ok(metadata) => (format!("{}|{}", filename_lower, metadata.len()), false),
        Err(_) => (filename_lower, true),
    }
}

fn select_winner(candidates: &[String], rules: &CompiledRules) -> Option<CandidateWinner> {
    if candidates.is_empty() {
        return None;
    }

    let mut scored: Vec<(u8, usize, String, String)> = candidates
        .iter()
        .map(|candidate| {
            let winner_type = classify_candidate(candidate, rules);
            let priority = match winner_type.as_str() {
                "IND" => 1u8,
                "SECONDARY" => 2u8,
                _ => 99u8,
            };
            (priority, candidate.len(), candidate.clone(), winner_type)
        })
        .collect();

    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    scored.first().map(|item| CandidateWinner {
        relpath: item.2.clone(),
        winner_type: item.3.clone(),
    })
}

fn classify_candidate(candidate: &str, rules: &CompiledRules) -> String {
    let lower = candidate.to_lowercase();
    if rules.ind_re.is_match(&lower) {
        return "IND".to_string();
    }
    if rules
        .secondary_tokens
        .iter()
        .any(|token| lower.contains(token))
    {
        return "SECONDARY".to_string();
    }
    "OTHER".to_string()
}

fn any_token_match(candidates: &[String], tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    if tokens.iter().any(|token| token == "*") {
        return true;
    }
    candidates.iter().any(|candidate| {
        let lower = candidate.to_lowercase();
        tokens.iter().any(|token| lower.contains(token))
    })
}

fn normalize_relpath(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy().to_string();
    rel_str.replace('\\', "/")
}

fn init_csv_writer(path: &Path) -> Result<csv::Writer<fs::File>, AppError> {
    let mut writer = WriterBuilder::new().has_headers(true).from_path(path)?;
    writer.write_record([
        "survey_id_base",
        "raw_relpath",
        "filename",
        "dolphin",
        "graded_relpath",
        "graded_hits",
        "graded_winner_type",
        "survey_id_raw_detected",
        "survey_id_graded_detected",
    ])?;
    Ok(writer)
}

fn write_rows_to_writer(
    writer: &mut csv::Writer<fs::File>,
    rows: &[CsvRow],
) -> Result<(), AppError> {
    for row in rows {
        writer.write_record([
            row.survey_id_base.as_str(),
            row.raw_relpath.as_str(),
            row.filename.as_str(),
            &row.dolphin.to_string(),
            row.graded_relpath.as_str(),
            &row.graded_hits.to_string(),
            row.graded_winner_type.as_str(),
            row.survey_id_raw_detected.as_deref().unwrap_or(""),
            row.survey_id_graded_detected.as_deref().unwrap_or(""),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

fn write_csv_rows(path: &Path, rows: &[CsvRow]) -> Result<(), AppError> {
    let mut writer = init_csv_writer(path)?;
    write_rows_to_writer(&mut writer, rows)
}

fn write_problems_csv(path: &Path, problems: &[ProblemItem]) -> Result<(), AppError> {
    let mut writer = WriterBuilder::new().has_headers(true).from_path(path)?;
    writer.write_record([
        "survey_id_base",
        "survey_id_detected",
        "raw_path",
        "graded_path",
        "problem_type",
        "details",
    ])?;
    for problem in problems {
        writer.write_record([
            problem.survey_id_base.as_str(),
            problem.survey_id_detected.as_deref().unwrap_or(""),
            problem.raw_path.as_deref().unwrap_or(""),
            problem.graded_path.as_deref().unwrap_or(""),
            problem.problem_type.as_str(),
            problem.details.as_deref().unwrap_or(""),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Clone, Debug)]
struct ScanEntry {
    base_key: String,
    raw: Option<SurveyFolder>,
    graded: Option<SurveyFolder>,
    status: String,
}

#[derive(Clone, Debug)]
struct ScanResult {
    entries: Vec<ScanEntry>,
    problems: Vec<ProblemItem>,
    preview: Vec<PreviewItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_detected_and_base() {
        let rules = Rules {
            extensions: vec![".jpg".to_string()],
            survey_id_regex_detected: "(?i)\\b(\\d{8}_[A-Z]{2}(?:_[A-Z]{2})?)\\b".to_string(),
            survey_id_regex_base: "(?i)\\b(\\d{8}_[A-Z]{2})(?:_[A-Z]{2})?\\b".to_string(),
            image_id_regex: default_image_id_regex(),
            graded_priority_ind_regex: "(?i)\\bind".to_string(),
            graded_priority_secondary_tokens: vec!["best".to_string()],
            graded_negative_contains_any: vec![],
            graded_positive_contains_any: vec!["*".to_string()],
        };
        let compiled = compile_rules(&rules).expect("compile");

        let path = PathBuf::from("/data/20250101_AB_CD/some");
        let detected = extract_detected_id(&path, &compiled.detected_re).expect("detected");
        let base = extract_base_key(&detected, &compiled.base_re).expect("base");
        assert_eq!(detected, "20250101_AB_CD");
        assert_eq!(base, "20250101_AB");
    }

    #[test]
    fn winner_selection_prefers_ind_then_secondary() {
        let rules = Rules {
            extensions: vec![".jpg".to_string()],
            survey_id_regex_detected: "x".to_string(),
            survey_id_regex_base: "x".to_string(),
            image_id_regex: default_image_id_regex(),
            graded_priority_ind_regex: "(?i)\\bind".to_string(),
            graded_priority_secondary_tokens: vec!["best".to_string()],
            graded_negative_contains_any: vec![],
            graded_positive_contains_any: vec!["*".to_string()],
        };
        let compiled = compile_rules(&rules).expect("compile");
        let candidates = vec![
            "alpha/best/image.jpg".to_string(),
            "beta/ind/image.jpg".to_string(),
            "gamma/other/image.jpg".to_string(),
        ];
        let winner = select_winner(&candidates, &compiled).expect("winner");
        assert_eq!(winner.relpath, "beta/ind/image.jpg");
        assert_eq!(winner.winner_type, "IND");
    }

    #[test]
    fn file_id_uses_size_when_available() {
        let rules = Rules {
            extensions: vec![".jpg".to_string()],
            survey_id_regex_detected: "x".to_string(),
            survey_id_regex_base: "x".to_string(),
            image_id_regex: "^no-match$".to_string(),
            graded_priority_ind_regex: "(?i)\\bind".to_string(),
            graded_priority_secondary_tokens: vec!["best".to_string()],
            graded_negative_contains_any: vec![],
            graded_positive_contains_any: vec!["*".to_string()],
        };
        let compiled = compile_rules(&rules).expect("compile");
        let temp_dir = std::env::temp_dir().join("survey_labeler_test");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("sample.JPG");
        fs::write(&file_path, b"testdata").expect("write");

        let (file_id, ambiguous) = compute_file_id(&file_path, &compiled);
        assert!(file_id.starts_with("sample.jpg|"));
        assert!(!ambiguous);
    }

    #[test]
    fn file_id_strips_suffix_tokens() {
        let rules = Rules {
            extensions: vec![".jpg".to_string()],
            survey_id_regex_detected: "x".to_string(),
            survey_id_regex_base: "x".to_string(),
            image_id_regex: "^(.+?_\\d{3,5})(?:_[A-Za-z0-9]+)*$".to_string(),
            graded_priority_ind_regex: "(?i)\\bind".to_string(),
            graded_priority_secondary_tokens: vec!["best".to_string()],
            graded_negative_contains_any: vec![],
            graded_positive_contains_any: vec!["*".to_string()],
        };
        let compiled = compile_rules(&rules).expect("compile");
        let file_path = PathBuf::from("/data/20100428_ALA_0449_QP_D.jpg");
        let (file_id, ambiguous) = compute_file_id(&file_path, &compiled);
        assert_eq!(file_id, "20100428_ala_0449");
        assert!(!ambiguous);
    }
}
