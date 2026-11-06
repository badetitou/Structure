//! Tauri commands: thin wrappers over `structure_core::pipeline`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use structure_core::model::{History, Impact, Matrix, Progress};
use structure_core::pipeline::{self, Analysis, Options};
use tauri::{AppHandle, Emitter, Manager};

/// Cached analyses keyed by canonical project path.
#[derive(Default)]
pub struct Cache(pub Mutex<HashMap<String, Analysis>>);

fn emit_progress(app: &AppHandle, step: &str, pct: u8) {
    let _ = app.emit(
        "analysis-progress",
        Progress {
            step: step.to_string(),
            pct,
        },
    );
}

fn canonical(path: &str) -> Result<(PathBuf, String), String> {
    let p = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("cannot open {path}: {e}"))?;
    let key = p.to_string_lossy().into_owned();
    Ok((p, key))
}

/// Analyze (warnings travel in `Matrix::warnings`), emit progress, and cache
/// the result.
fn run_analyze(app: &AppHandle, path: &Path, key: &str, opts: &Options) -> Result<Matrix, String> {
    let a = pipeline::analyze(path, opts, &|p| emit_progress(app, &p.step, p.pct))
        .map_err(|e| format!("{e:#}"))?;
    let matrix = a.matrix.clone();
    app.state::<Cache>()
        .0
        .lock()
        .map_err(|_| "internal error: analysis cache poisoned".to_string())?
        .insert(key.to_string(), a);
    Ok(matrix)
}

#[tauri::command]
pub async fn analyze(app: AppHandle, path: String) -> Result<Matrix, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, key) = canonical(&path)?;
        // An explicit Analyze re-indexes when sources are newer than index.scip.
        run_analyze(&app, &p, &key, &Options::default())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn impact(
    app: AppHandle,
    path: String,
    base: Option<String>,
    head: Option<String>,
) -> Result<Impact, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, key) = canonical(&path)?;
        // Always re-parse: the user typically edits files between Analyze and
        // Show impact, and symbol ranges must match the diff. Re-parsing takes
        // well under a second; the (possibly stale) index is reused.
        let opts = Options {
            reindex_if_stale: false,
            ..Options::default()
        };
        run_analyze(&app, &p, &key, &opts)?;
        emit_progress(&app, "computing impact", 50);
        let result = {
            let state = app.state::<Cache>();
            let guard = state
                .0
                .lock()
                .map_err(|_| "internal error: analysis cache poisoned".to_string())?;
            let a = guard
                .get(&key)
                .ok_or_else(|| "analysis missing from cache".to_string())?;
            pipeline::impact(&p, a, base.as_deref(), head.as_deref())
                .map_err(|e| format!("{e:#}"))?
        };
        emit_progress(&app, "done", 100);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn history(path: String, limit: Option<u32>) -> Result<History, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, _) = canonical(&path)?;
        pipeline::history(&p, limit.unwrap_or(200)).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_reports_missing_path() {
        assert!(canonical("/definitely/not/here").is_err());
    }
}
