import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { open as openShell } from '@tauri-apps/plugin-shell'

type Rules = {
  extensions: string[]
  survey_id_regex_detected: string
  survey_id_regex_base: string
  image_id_regex: string
  graded_priority_ind_regex: string
  graded_priority_secondary_tokens: string[]
  graded_negative_contains_any: string[]
  graded_positive_contains_any: string[]
}

type RootRunOptions = {
  write_per_survey: boolean
  write_merged: boolean
  merged_filename: string
  problems_filename: string
  per_survey_dirname: string
}

type SingleRunOptions = {
  output_filename: string
}

type PreviewItem = {
  base_key: string
  raw_path: string | null
  graded_path: string | null
  status: string
  problem_type: string | null
  details: string | null
  raw_image_count: number | null
  graded_image_count: number | null
  survey_id_raw_detected: string | null
  survey_id_graded_detected: string | null
}

type RunSummary = {
  processed_surveys: number
  total_rows: number
  dolphin_yes: number
  dolphin_no: number
  ambiguity_warnings: number
  problems_count: number
  output_dir: string
  merged_csv_path: string | null
  problems_csv_path: string | null
}

type ProgressEvent = {
  survey_id_base: string
  processed: number
  total: number
}

type Mode = 'root' | 'single' | 'settings'

const listToText = (list: string[]) => list.join('\n')
const textToList = (value: string) =>
  value
    .split(/\r?\n|,/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0)

const defaultRootOptions: RootRunOptions = {
  write_per_survey: true,
  write_merged: true,
  merged_filename: 'merged.csv',
  problems_filename: 'problems.csv',
  per_survey_dirname: 'per_survey'
}

const defaultSingleOptions: SingleRunOptions = {
  output_filename: 'single.csv'
}

const PathField = ({
  label,
  value,
  onChange,
  onBrowse,
  browseLabel
}: {
  label: string
  value: string
  onChange: (value: string) => void
  onBrowse: () => void
  browseLabel: string
}) => (
  <label className="field">
    <span>{label}</span>
    <div className="field-row">
      <input value={value} onChange={(event) => onChange(event.target.value)} placeholder="/" />
      <button type="button" className="secondary" onClick={onBrowse}>
        {browseLabel}
      </button>
    </div>
  </label>
)

const SectionTitle = ({ title }: { title: string }) => <h2>{title}</h2>

function App() {
  const { t, i18n } = useTranslation()
  const [mode, setMode] = useState<Mode>('root')
  const [rules, setRules] = useState<Rules | null>(null)
  const [draftRules, setDraftRules] = useState<Rules | null>(null)
  const [statusMessage, setStatusMessage] = useState<string | null>(null)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const [gradedRoot, setGradedRoot] = useState('')
  const [rawRoot, setRawRoot] = useState('')
  const [outputDir, setOutputDir] = useState('')
  const [rootOptions, setRootOptions] = useState<RootRunOptions>(defaultRootOptions)
  const [preview, setPreview] = useState<PreviewItem[]>([])
  const [summary, setSummary] = useState<RunSummary | null>(null)
  const [progress, setProgress] = useState<ProgressEvent | null>(null)

  const [singleGradedDir, setSingleGradedDir] = useState('')
  const [singleRawDir, setSingleRawDir] = useState('')
  const [singleOutputDir, setSingleOutputDir] = useState('')
  const [singleOverride, setSingleOverride] = useState('')
  const [singleOptions, setSingleOptions] = useState<SingleRunOptions>(defaultSingleOptions)

  useEffect(() => {
    const load = async () => {
      try {
        const loaded = await invoke<Rules>('get_config')
        setRules(loaded)
        setDraftRules(loaded)
      } catch (err) {
        setErrorMessage(String(err))
      }
    }
    load()
  }, [])

  useEffect(() => {
    const unlistenPromise = listen<ProgressEvent>('progress', (event) => {
      setProgress(event.payload)
    })
    return () => {
      unlistenPromise.then((unlisten) => unlisten())
    }
  }, [])

  const languageOptions = useMemo(
    () => [
      { value: 'en', label: 'English' },
      { value: 'fr', label: 'Francais' },
      { value: 'de', label: 'Deutsch' }
    ],
    []
  )

  const pickFolder = async (setter: (value: string) => void) => {
    const selected = await openDialog({ directory: true, multiple: false })
    if (typeof selected === 'string') {
      setter(selected)
    }
  }

  const handlePreview = async () => {
    if (!rules) return
    setBusy(true)
    setErrorMessage(null)
    setSummary(null)
    setProgress(null)
    try {
      const result = await invoke<PreviewItem[]>('preview_root_scan_cmd', {
        gradedRoot,
        rawRoot,
        config: rules
      })
      setPreview(result)
    } catch (err) {
      setErrorMessage(String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleRunRoot = async () => {
    if (!rules) return
    setBusy(true)
    setErrorMessage(null)
    setStatusMessage(null)
    setSummary(null)
    setProgress(null)
    try {
      const result = await invoke<RunSummary>('run_root_scan_cmd', {
        gradedRoot,
        rawRoot,
        outputDir,
        options: rootOptions,
        config: rules
      })
      setSummary(result)
    } catch (err) {
      setErrorMessage(String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleRunSingle = async () => {
    if (!rules) return
    setBusy(true)
    setErrorMessage(null)
    setStatusMessage(null)
    setSummary(null)
    setProgress(null)
    try {
      const result = await invoke<RunSummary>('run_single_pair_cmd', {
        gradedDir: singleGradedDir,
        rawDir: singleRawDir,
        outputDir: singleOutputDir,
        surveyIdOverride: singleOverride.length > 0 ? singleOverride : null,
        options: singleOptions,
        config: rules
      })
      setSummary(result)
    } catch (err) {
      setErrorMessage(String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleOpenOutput = async (path: string) => {
    setErrorMessage(null)
    try {
      await openShell(path)
    } catch (err) {
      setErrorMessage(String(err))
    }
  }

  const handleSaveRules = async () => {
    if (!draftRules) return
    setBusy(true)
    setErrorMessage(null)
    try {
      const saved = await invoke<Rules>('save_config', { rules: draftRules })
      setRules(saved)
      setDraftRules(saved)
      setStatusMessage(t('settings.saveSuccess'))
    } catch (err) {
      setErrorMessage(String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleResetRules = async () => {
    setBusy(true)
    setErrorMessage(null)
    try {
      const saved = await invoke<Rules>('reset_config')
      setRules(saved)
      setDraftRules(saved)
      setStatusMessage(t('settings.resetSuccess'))
    } catch (err) {
      setErrorMessage(String(err))
    } finally {
      setBusy(false)
    }
  }

  const settingsView = draftRules ? (
    <section className="panel">
      <SectionTitle title={t('settings.title')} />
      <div className="grid">
        <label className="field">
          <span>{t('settings.extensions')}</span>
          <textarea
            value={listToText(draftRules.extensions)}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                extensions: textToList(event.target.value)
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.detectedRegex')}</span>
          <input
            value={draftRules.survey_id_regex_detected}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                survey_id_regex_detected: event.target.value
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.baseRegex')}</span>
          <input
            value={draftRules.survey_id_regex_base}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                survey_id_regex_base: event.target.value
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.imageIdRegex')}</span>
          <input
            value={draftRules.image_id_regex}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                image_id_regex: event.target.value
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.indRegex')}</span>
          <input
            value={draftRules.graded_priority_ind_regex}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                graded_priority_ind_regex: event.target.value
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.secondaryTokens')}</span>
          <textarea
            value={listToText(draftRules.graded_priority_secondary_tokens)}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                graded_priority_secondary_tokens: textToList(event.target.value)
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.negativeTokens')}</span>
          <textarea
            value={listToText(draftRules.graded_negative_contains_any)}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                graded_negative_contains_any: textToList(event.target.value)
              })
            }
          />
        </label>
        <label className="field">
          <span>{t('settings.positiveTokens')}</span>
          <textarea
            value={listToText(draftRules.graded_positive_contains_any)}
            onChange={(event) =>
              setDraftRules({
                ...draftRules,
                graded_positive_contains_any: textToList(event.target.value)
              })
            }
          />
        </label>
      </div>
      <p className="help">{t('settings.helperImageIdRegex')}</p>
      <p className="help">{t('settings.helperTokens')}</p>
      <div className="actions">
        <button onClick={handleSaveRules} disabled={busy}>
          {t('common.save')}
        </button>
        <button className="secondary" onClick={handleResetRules} disabled={busy}>
          {t('common.reset')}
        </button>
      </div>
    </section>
  ) : null

  return (
    <div className="app">
      <header className="header">
        <div>
          <h1>{t('app.title')}</h1>
          <p>{t('app.subtitle')}</p>
        </div>
        <div className="lang-select">
          <span>{t('common.language')}</span>
          <select value={i18n.language} onChange={(event) => i18n.changeLanguage(event.target.value)}>
            {languageOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </header>

      <nav className="tabs">
        <button className={mode === 'root' ? 'active' : ''} onClick={() => setMode('root')}>
          {t('nav.root')}
        </button>
        <button className={mode === 'single' ? 'active' : ''} onClick={() => setMode('single')}>
          {t('nav.single')}
        </button>
        <button className={mode === 'settings' ? 'active' : ''} onClick={() => setMode('settings')}>
          {t('nav.settings')}
        </button>
      </nav>

      {errorMessage && (
        <div className="notice error">
          <strong>{t('common.error')}:</strong> {errorMessage}
        </div>
      )}
      {statusMessage && <div className="notice">{statusMessage}</div>}

      {mode === 'root' && (
        <section className="panel">
          <SectionTitle title={t('root.title')} />
          <div className="grid">
            <PathField
              label={t('root.gradedRoot')}
              value={gradedRoot}
              onChange={setGradedRoot}
              onBrowse={() => pickFolder(setGradedRoot)}
              browseLabel={t('common.browse')}
            />
            <PathField
              label={t('root.rawRoot')}
              value={rawRoot}
              onChange={setRawRoot}
              onBrowse={() => pickFolder(setRawRoot)}
              browseLabel={t('common.browse')}
            />
            <PathField
              label={t('root.outputDir')}
              value={outputDir}
              onChange={setOutputDir}
              onBrowse={() => pickFolder(setOutputDir)}
              browseLabel={t('common.browse')}
            />
          </div>

          <div className="options">
            <h3>{t('root.options')}</h3>
            <label className="toggle">
              <input
                type="checkbox"
                checked={rootOptions.write_per_survey}
                onChange={(event) =>
                  setRootOptions({
                    ...rootOptions,
                    write_per_survey: event.target.checked
                  })
                }
              />
              <span>{t('root.writePerSurvey')}</span>
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={rootOptions.write_merged}
                onChange={(event) =>
                  setRootOptions({
                    ...rootOptions,
                    write_merged: event.target.checked
                  })
                }
              />
              <span>{t('root.writeMerged')}</span>
            </label>
            <div className="grid">
              <label className="field">
                <span>{t('root.mergedFilename')}</span>
                <input
                  value={rootOptions.merged_filename}
                  onChange={(event) =>
                    setRootOptions({
                      ...rootOptions,
                      merged_filename: event.target.value
                    })
                  }
                />
              </label>
              <label className="field">
                <span>{t('root.problemsFilename')}</span>
                <input
                  value={rootOptions.problems_filename}
                  onChange={(event) =>
                    setRootOptions({
                      ...rootOptions,
                      problems_filename: event.target.value
                    })
                  }
                />
              </label>
              <label className="field">
                <span>{t('root.perSurveyDirname')}</span>
                <input
                  value={rootOptions.per_survey_dirname}
                  onChange={(event) =>
                    setRootOptions({
                      ...rootOptions,
                      per_survey_dirname: event.target.value
                    })
                  }
                />
              </label>
            </div>
          </div>

          <div className="actions">
            <button onClick={handlePreview} disabled={busy || !rules || !gradedRoot || !rawRoot}>
              {t('common.preview')}
            </button>
            <button
              className="primary"
              onClick={handleRunRoot}
              disabled={busy || !rules || !gradedRoot || !rawRoot || !outputDir}
            >
              {t('common.run')}
            </button>
          </div>

          {progress && (
            <div className="progress">
              <div>
                <strong>{t('progress.label')}:</strong> {progress.survey_id_base}
              </div>
              <div>
                {t('progress.filesProcessed')}: {progress.processed}/{progress.total}
              </div>
            </div>
          )}

          <div className="preview">
            <h3>{t('root.previewTitle')}</h3>
            {preview.length === 0 ? (
              <p className="muted">{t('root.noPreview')}</p>
            ) : (
              <div className="table">
                <div className="row head">
                  <span>{t('root.table.baseKey')}</span>
                  <span>{t('root.table.rawPath')}</span>
                  <span>{t('root.table.gradedPath')}</span>
                  <span>{t('root.table.status')}</span>
                  <span>{t('root.table.rawCount')}</span>
                  <span>{t('root.table.gradedCount')}</span>
                </div>
                {preview.map((item) => (
                  <div className={`row ${item.status === 'OK' ? 'ok' : 'problem'}`} key={item.base_key}>
                    <span>{item.base_key}</span>
                    <span title={item.raw_path ?? ''}>{item.raw_path ?? '-'}</span>
                    <span title={item.graded_path ?? ''}>{item.graded_path ?? '-'}</span>
                    <span>{item.status}</span>
                    <span>{item.raw_image_count ?? '-'}</span>
                    <span>{item.graded_image_count ?? '-'}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>
      )}

      {mode === 'single' && (
        <section className="panel">
          <SectionTitle title={t('single.title')} />
          <div className="grid">
            <PathField
              label={t('single.gradedDir')}
              value={singleGradedDir}
              onChange={setSingleGradedDir}
              onBrowse={() => pickFolder(setSingleGradedDir)}
              browseLabel={t('common.browse')}
            />
            <PathField
              label={t('single.rawDir')}
              value={singleRawDir}
              onChange={setSingleRawDir}
              onBrowse={() => pickFolder(setSingleRawDir)}
              browseLabel={t('common.browse')}
            />
            <PathField
              label={t('single.outputDir')}
              value={singleOutputDir}
              onChange={setSingleOutputDir}
              onBrowse={() => pickFolder(setSingleOutputDir)}
              browseLabel={t('common.browse')}
            />
            <label className="field">
              <span>{t('single.surveyOverride')}</span>
              <input value={singleOverride} onChange={(event) => setSingleOverride(event.target.value)} />
            </label>
            <label className="field">
              <span>{t('single.outputFilename')}</span>
              <input
                value={singleOptions.output_filename}
                onChange={(event) =>
                  setSingleOptions({
                    ...singleOptions,
                    output_filename: event.target.value
                  })
                }
              />
            </label>
          </div>

          <div className="actions">
            <button
              className="primary"
              onClick={handleRunSingle}
              disabled={busy || !rules || !singleGradedDir || !singleRawDir || !singleOutputDir}
            >
              {t('common.run')}
            </button>
          </div>

          {progress && (
            <div className="progress">
              <div>
                <strong>{t('progress.label')}:</strong> {progress.survey_id_base}
              </div>
              <div>
                {t('progress.filesProcessed')}: {progress.processed}/{progress.total}
              </div>
            </div>
          )}
        </section>
      )}

      {mode === 'settings' && settingsView}

      {summary && (
        <section className="panel summary">
          <SectionTitle title={t('summary.title')} />
          <div className="summary-grid">
            <div>
              <span>{t('summary.processedSurveys')}</span>
              <strong>{summary.processed_surveys}</strong>
            </div>
            <div>
              <span>{t('summary.totalRows')}</span>
              <strong>{summary.total_rows}</strong>
            </div>
            <div>
              <span>{t('summary.dolphinYes')}</span>
              <strong>{summary.dolphin_yes}</strong>
            </div>
            <div>
              <span>{t('summary.dolphinNo')}</span>
              <strong>{summary.dolphin_no}</strong>
            </div>
            <div>
              <span>{t('summary.problemsCount')}</span>
              <strong>{summary.problems_count}</strong>
            </div>
            <div>
              <span>{t('summary.ambiguityWarnings')}</span>
              <strong>{summary.ambiguity_warnings}</strong>
            </div>
          </div>
          <div className="summary-links">
            {summary.merged_csv_path && (
              <div>
                <span>{t('summary.mergedCsv')}:</span>
                <code>{summary.merged_csv_path}</code>
              </div>
            )}
            {summary.problems_csv_path && (
              <div>
                <span>{t('summary.problemsCsv')}:</span>
                <code>{summary.problems_csv_path}</code>
              </div>
            )}
          </div>
          <div className="actions">
            <button
              className="secondary"
              onClick={() => summary.output_dir && handleOpenOutput(summary.output_dir)}
            >
              {t('common.openOutput')}
            </button>
          </div>
        </section>
      )}
    </div>
  )
}

export default App
