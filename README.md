# Survey Label Exporter

Cross-platform desktop app (Tauri v2 + React + TypeScript) that labels raw survey images as Dolphin Yes/No by matching against graded folders.

## Features
- Root Scan mode: pick Graded root, Raw root, Output folder, preview matching, then run.
- Single Pair mode: process one graded/raw pair with optional survey ID override.
- Merged CSV + per-survey CSVs + problems.csv.
- Configurable rules via in-app Settings (stored in app data directory).
- Built-in i18n (English, French, German).

## Development

Prerequisites:
- Node.js 18+
- Rust toolchain (stable)
- Tauri prerequisites for your OS (see Tauri docs)

Install dependencies:

```bash
npm install
```

Run the desktop app (Tauri dev):

```bash
npm run dev:tauri
```

## Build

macOS:

```bash
npm run build:tauri
```

Windows (from Windows machine or CI with Windows):

```bash
npm run build:tauri
```

## Rules Configuration

Rules are stored in the platform app data directory as `rules.json`. The app will copy defaults on first run. You can edit rules in the Settings tab.

Default rules live in `src-tauri/assets/rules.default.json`.

## Sample Data

Generate a small dummy dataset:

```bash
npm run sample-data
```

This creates `sample-data/Raw` and `sample-data/Graded` with a few surveys and problems.

## CSV Schema

Merged and per-survey CSVs include:

- `survey_id_base`
- `raw_relpath`
- `filename`
- `dolphin`
- `graded_relpath`
- `graded_hits`
- `graded_winner_type`
- `survey_id_raw_detected`
- `survey_id_graded_detected`

Problems CSV includes:

- `survey_id_base`
- `survey_id_detected`
- `raw_path`
- `graded_path`
- `problem_type`
- `details`

## Rust Tests

```bash
cd src-tauri
cargo test
```
