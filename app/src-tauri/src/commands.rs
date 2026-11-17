//! Tauri commands: thin wrappers over `structure_core::session::Session`
//! (managed state). They run it off the async runtime, emit its progress as
//! `analysis-progress` events and turn its errors into strings.

use structure_core::model::{
    CellSymbols, Diff, History, Impact, ImpactedSymbol, LensCells, LensInfo, Matrix, Progress,
};
use structure_core::pipeline::Options;
use structure_core::session::Session;
use tauri::{AppHandle, Emitter, Manager};

fn emit_progress(app: &AppHandle, p: Progress) {
    let _ = app.emit("analysis-progress", p);
}

/// Run `f` on the app's `Session` in a blocking thread; errors read `{e:#}`.
async fn with_session<T: Send + 'static, E: std::fmt::Display>(
    app: AppHandle,
    f: impl FnOnce(&AppHandle, &Session) -> Result<T, E> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        f(&app, &app.state::<Session>()).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn analyze(app: AppHandle, paths: Vec<String>) -> Result<Matrix, String> {
    // An explicit Analyze re-indexes when sources are newer than index.scip.
    with_session(app, move |app, s| {
        s.analyze(&paths, &Options::default(), &|p| emit_progress(app, p))
    })
    .await
}

/// Re-apply the projects' rules to the cached analysis of `paths` (no
/// re-indexing, no re-parsing); the matrix carries a new generation.
#[tauri::command]
pub async fn reclassify(app: AppHandle, paths: Vec<String>) -> Result<Matrix, String> {
    with_session(app, move |_, s| s.reclassify(&paths)).await
}

/// Opaque stamp of the projects' rules files: the UI polls it and calls
/// `reclassify` when it changes.
#[tauri::command]
pub async fn rules_stamp(app: AppHandle, paths: Vec<String>) -> Result<String, String> {
    with_session(app, move |_, s| s.rules_stamp(&paths)).await
}

#[tauri::command]
pub async fn impact(
    app: AppHandle,
    path: String,
    base: Option<String>,
    head: Option<String>,
) -> Result<Impact, String> {
    with_session(app, move |app, s| {
        s.impact(&path, base.as_deref(), head.as_deref(), &|p| {
            emit_progress(app, p)
        })
    })
    .await
}

/// Symbols of the given cells, paged. `generation` is `Matrix::generation` of
/// the matrix on screen; `head` is the full SHA from `Impact::head`, or null
/// for the working tree; `lens` the lens of the cell ids (null: the default).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cell_symbols(
    app: AppHandle,
    paths: Vec<String>,
    head: Option<String>,
    generation: u64,
    lens: Option<String>,
    cells: Vec<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<CellSymbols, String> {
    with_session(app, move |_, s| {
        s.cell_symbols(
            &paths,
            head.as_deref(),
            generation,
            lens.as_deref(),
            &cells,
            offset,
            limit,
        )
    })
    .await
}

/// The lenses a matrix can be requested in, the default first.
#[tauri::command]
pub fn lenses() -> Vec<LensInfo> {
    Session::lenses()
}

/// The matrix of a cached analysis (same `paths`, `head` and `generation` as
/// `cell_symbols`) through lens `lens`.
#[tauri::command]
pub async fn matrix(
    app: AppHandle,
    paths: Vec<String>,
    head: Option<String>,
    generation: u64,
    lens: String,
) -> Result<Matrix, String> {
    with_session(app, move |_, s| {
        s.matrix(&paths, head.as_deref(), generation, &lens)
    })
    .await
}

/// The cells of an impact of `path` (`head` = `Impact::head`) in lens `lens`,
/// from its changed and impacted symbols.
#[tauri::command]
pub async fn impact_cells(
    app: AppHandle,
    path: String,
    head: Option<String>,
    lens: String,
    changed: Vec<String>,
    impacted: Vec<ImpactedSymbol>,
) -> Result<LensCells, String> {
    with_session(app, move |_, s| {
        s.impact_cells(&path, head.as_deref(), &lens, &changed, &impacted)
    })
    .await
}

#[tauri::command]
pub async fn history(app: AppHandle, path: String, limit: Option<u32>) -> Result<History, String> {
    with_session(app, move |_, s| s.history(&path, limit)).await
}

/// Textual diff of `base..head` (same meaning as `impact`), bounded in size.
#[tauri::command]
pub async fn diff(
    app: AppHandle,
    path: String,
    base: Option<String>,
    head: Option<String>,
) -> Result<Diff, String> {
    with_session(app, move |_, s| {
        s.diff(&path, base.as_deref(), head.as_deref())
    })
    .await
}
