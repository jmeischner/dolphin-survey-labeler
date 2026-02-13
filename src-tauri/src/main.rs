use std::path::PathBuf;

use survey_labeler::{
    get_or_init_rules, preview_root_scan, reset_rules, run_root_scan, run_single_pair, save_rules,
    RootRunOptions, Rules, SingleRunOptions,
};

#[tauri::command]
fn get_config(app: tauri::AppHandle) -> Result<Rules, String> {
    get_or_init_rules(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, rules: Rules) -> Result<Rules, String> {
    save_rules(&app, rules).map_err(|err| err.to_string())
}

#[tauri::command]
fn reset_config(app: tauri::AppHandle) -> Result<Rules, String> {
    reset_rules(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn preview_root_scan_cmd(
    graded_root: String,
    raw_root: String,
    config: Option<Rules>,
    app: tauri::AppHandle,
) -> Result<Vec<survey_labeler::PreviewItem>, String> {
    let rules = match config {
        Some(rules) => rules,
        None => get_or_init_rules(&app).map_err(|err| err.to_string())?,
    };
    preview_root_scan(PathBuf::from(graded_root), PathBuf::from(raw_root), rules)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn run_root_scan_cmd(
    graded_root: String,
    raw_root: String,
    output_dir: String,
    options: RootRunOptions,
    config: Option<Rules>,
    app: tauri::AppHandle,
) -> Result<survey_labeler::RunSummary, String> {
    let rules = match config {
        Some(rules) => rules,
        None => get_or_init_rules(&app).map_err(|err| err.to_string())?,
    };
    run_root_scan(
        &app,
        PathBuf::from(graded_root),
        PathBuf::from(raw_root),
        PathBuf::from(output_dir),
        options,
        rules,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn run_single_pair_cmd(
    graded_dir: String,
    raw_dir: String,
    output_dir: String,
    survey_id_override: Option<String>,
    options: SingleRunOptions,
    config: Option<Rules>,
    app: tauri::AppHandle,
) -> Result<survey_labeler::RunSummary, String> {
    let rules = match config {
        Some(rules) => rules,
        None => get_or_init_rules(&app).map_err(|err| err.to_string())?,
    };
    run_single_pair(
        &app,
        PathBuf::from(graded_dir),
        PathBuf::from(raw_dir),
        PathBuf::from(output_dir),
        survey_id_override,
        options,
        rules,
    )
    .map_err(|err| err.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            reset_config,
            preview_root_scan_cmd,
            run_root_scan_cmd,
            run_single_pair_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
