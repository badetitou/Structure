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
    emit_progress_event(
        app,
        Progress {
            step: step.to_string(),
            pct,
            detail: None,
        },
    );
}

fn emit_progress_event(app: &AppHandle, p: Progress) {
    let _ = app.emit("analysis-progress", p);
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
    let a = pipeline::analyze(path, opts, &|p| emit_progress_event(app, p))
        .map_err(|e| format!("{e:#}"))?;
    let matrix = a.matrix.clone();
    app.state::<Cache>()
        .0
        .lock()
        .map_err(|_| "internal error: analysis cache poisoned".to_string())?
        .insert(key.to_string(), a);
    Ok(matrix)
}

/// Display form of a revision: 7 chars for a full hex SHA, else as given.
fn short_rev(rev: &str) -> &str {
    if rev.len() >= 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
        &rev[..7]
    } else {
        rev
    }
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
        if let Some(h) = head.as_deref() {
            // A commit is analysed in its own cached checkout; the working-tree
            // entry in `Cache` is left alone. Re-runs are cheap because the
            // core caches the snapshot and its index.scip (no extra in-memory
            // cache here).
            emit_progress(&app, &format!("checking out {}", short_rev(h)), 2);
            let opts = Options::default();
            let result = pipeline::impact_at(&p, base.as_deref(), Some(h), &opts, &|pr| {
                emit_progress_event(&app, pr)
            })
            .map_err(|e| format!("{e:#}"))?;
            emit_progress(&app, "done", 100);
            return Ok(result);
        }
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
    fn short_rev_shortens_only_full_shas() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(short_rev(sha), "0123456");
        assert_eq!(short_rev("main"), "main");
    }

    #[test]
    fn canonical_reports_missing_path() {
        assert!(canonical("/definitely/not/here").is_err());
    }
}
