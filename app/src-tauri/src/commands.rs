//! Tauri commands. To go real: replace the `mock::*` calls below with calls
//! into `structure_core` (keep them inside `spawn_blocking`).

use structure_core::model::{Impact, Matrix, Progress};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub async fn analyze(app: AppHandle, path: String) -> Result<Matrix, String> {
    tauri::async_runtime::spawn_blocking(move || mock::analyze(&app, &path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn impact(
    path: String,
    base: Option<String>,
    head: Option<String>,
) -> Result<Impact, String> {
    tauri::async_runtime::spawn_blocking(move || mock::impact(&path, base, head))
        .await
        .map_err(|e| e.to_string())?
}

fn emit_progress(app: &AppHandle, step: &str, pct: u8) {
    let _ = app.emit(
        "analysis-progress",
        Progress {
            step: step.to_string(),
            pct,
        },
    );
}

/// Mocked backend: returns the sample files from `docs/samples`.
mod mock {
    use super::*;
    use std::{thread::sleep, time::Duration};

    const MATRIX: &str = include_str!("../../../docs/samples/matrix.json");
    const IMPACT: &str = include_str!("../../../docs/samples/impact.json");

    pub fn analyze(app: &AppHandle, _path: &str) -> Result<Matrix, String> {
        for (step, pct) in [
            ("indexing", 10),
            ("parsing", 40),
            ("linking", 65),
            ("classifying", 85),
            ("building matrix", 100),
        ] {
            emit_progress(app, step, pct);
            sleep(Duration::from_millis(350));
        }
        serde_json::from_str(MATRIX).map_err(|e| e.to_string())
    }

    pub fn impact(
        _path: &str,
        _base: Option<String>,
        _head: Option<String>,
    ) -> Result<Impact, String> {
        serde_json::from_str(IMPACT).map_err(|e| e.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn samples_parse() {
            assert!(serde_json::from_str::<Matrix>(MATRIX).is_ok());
            assert!(serde_json::from_str::<Impact>(IMPACT).is_ok());
        }
    }
}
