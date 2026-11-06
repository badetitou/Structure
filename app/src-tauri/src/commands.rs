//! Tauri commands: thin wrappers over `structure_core::pipeline`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use structure_core::matrix;
use structure_core::model::{CellSymbols, History, Impact, Matrix, Progress};
use structure_core::pipeline::{self, Analysis, Options};
use tauri::{AppHandle, Emitter, Manager};

/// One cached analysis: the working tree of a project (`fingerprint` set) or
/// the immutable snapshot of a commit.
struct Entry {
    key: String,
    /// Directory that was analysed (the project, or the commit's snapshot).
    dir: PathBuf,
    /// `pipeline::worktree_fingerprint` taken before the analysis.
    fingerprint: Option<String>,
    analysis: Arc<Analysis>,
}

/// Most recently used analyses kept in memory (a vscode graph is large).
const CACHE_CAPACITY: usize = 4;

/// Cached analyses, most recently used last. Working-tree entries are keyed
/// by canonical project path, commit snapshots by `<path>\0<head>`.
#[derive(Default)]
pub struct Cache(Mutex<Vec<Entry>>);

/// Hit: directory analysed, fingerprint, analysis.
type Hit = (PathBuf, Option<String>, Arc<Analysis>);

impl Cache {
    fn with<T>(&self, f: impl FnOnce(&mut Vec<Entry>) -> T) -> Result<T, String> {
        let mut g = self
            .0
            .lock()
            .map_err(|_| "internal error: analysis cache poisoned".to_string())?;
        Ok(f(&mut g))
    }

    /// The entry for `key`, marked as most recently used.
    fn get(&self, key: &str) -> Result<Option<Hit>, String> {
        self.with(|v| {
            let i = v.iter().position(|e| e.key == key)?;
            let e = v.remove(i);
            let out = (e.dir.clone(), e.fingerprint.clone(), e.analysis.clone());
            v.push(e);
            Some(out)
        })
    }

    fn put(&self, entry: Entry) -> Result<(), String> {
        self.with(|v| {
            v.retain(|e| e.key != entry.key);
            v.push(entry);
            while v.len() > CACHE_CAPACITY {
                v.remove(0);
            }
        })
    }
}

fn snapshot_key(key: &str, head: &str) -> String {
    format!("{key}\0{head}")
}

/// Serialises snapshot analyses: concurrent checkouts/indexing of the same
/// snapshot would race.
#[derive(Default)]
pub struct SnapshotLock(pub Mutex<()>);

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
fn run_analyze(
    app: &AppHandle,
    path: &Path,
    key: &str,
    opts: &Options,
) -> Result<Arc<Analysis>, String> {
    // Taken before the analysis: a change made meanwhile invalidates the entry.
    let fingerprint = pipeline::worktree_fingerprint(path).ok();
    let a = Arc::new(
        pipeline::analyze(path, opts, &|p| emit_progress_event(app, p))
            .map_err(|e| format!("{e:#}"))?,
    );
    app.state::<Cache>().put(Entry {
        key: key.to_string(),
        dir: path.to_path_buf(),
        fingerprint,
        analysis: a.clone(),
    })?;
    Ok(a)
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
        run_analyze(&app, &p, &key, &Options::default()).map(|a| a.matrix.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Impact of a commit: its snapshot is analysed once and cached.
fn impact_of_commit(
    app: &AppHandle,
    p: &Path,
    key: &str,
    base: Option<&str>,
    head: &str,
) -> Result<Impact, String> {
    let base_sha = base
        .map(|b| pipeline::resolve_base(p, b))
        .transpose()
        .map_err(|e| format!("{e:#}"))?;
    let skey = snapshot_key(key, head);
    let cache = app.state::<Cache>();
    let (dir, analysis) = match cache.get(&skey)? {
        Some((dir, _, a)) => (dir, a),
        None => {
            emit_progress(app, &format!("checking out {}", short_rev(head)), 2);
            let state = app.state::<SnapshotLock>();
            let _guard = state
                .0
                .lock()
                .map_err(|_| "internal error: snapshot lock poisoned".to_string())?;
            let (dir, a) =
                pipeline::analyze_snapshot(p, head, &Options::default(), &|pr| {
                    emit_progress_event(app, pr)
                })
                .map_err(|e| format!("{e:#}"))?;
            let a = Arc::new(a);
            cache.put(Entry {
                key: skey,
                dir: dir.clone(),
                fingerprint: None,
                analysis: a.clone(),
            })?;
            (dir, a)
        }
    };
    pipeline::impact_in_snapshot(&dir, &analysis, base_sha.as_deref(), head)
        .map_err(|e| format!("{e:#}"))
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
            let result = impact_of_commit(&app, &p, &key, base.as_deref(), h)?;
            emit_progress(&app, "done", 100);
            return Ok(result);
        }
        // The working-tree analysis is reused while nothing changed since
        // (same HEAD, status and file mtimes); otherwise re-parse so symbol
        // ranges match the diff. The (possibly stale) index is reused.
        let current = pipeline::worktree_fingerprint(&p).ok();
        let hit = match &current {
            Some(cur) => app
                .state::<Cache>()
                .get(&key)?
                .and_then(|(_, fp, a)| (fp.as_deref() == Some(cur.as_str())).then_some(a)),
            None => None,
        };
        let analysis = match hit {
            Some(a) => a,
            None => {
                let opts = Options {
                    reindex_if_stale: false,
                    ..Options::default()
                };
                run_analyze(&app, &p, &key, &opts)?
            }
        };
        emit_progress(&app, "computing impact", 50);
        let result = pipeline::impact(&p, &analysis, base.as_deref(), None)
            .map_err(|e| format!("{e:#}"))?;
        emit_progress(&app, "done", 100);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

const DEFAULT_PAGE: usize = 500;
const MAX_PAGE: usize = 2000;

/// Symbols of the given cells (the matrix carries counts only), paged.
#[tauri::command]
pub async fn cell_symbols(
    app: AppHandle,
    path: String,
    head: Option<String>,
    cells: Vec<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<CellSymbols, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, key) = canonical(&path)?;
        let key = match head.as_deref() {
            Some(h) => snapshot_key(&key, h),
            None => key,
        };
        let (_, _, a) = app
            .state::<Cache>()
            .get(&key)?
            .ok_or_else(|| "no analysis for this project: run Analyze first".to_string())?;
        Ok(matrix::cell_symbols(
            &a.graph,
            &a.classification,
            &cells,
            offset.unwrap_or(0),
            limit.unwrap_or(DEFAULT_PAGE).min(MAX_PAGE),
        ))
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

    #[test]
    fn snapshot_keys_differ_from_worktree_key() {
        assert_ne!(snapshot_key("/p", "abc"), "/p");
        assert_ne!(snapshot_key("/p", "abc"), snapshot_key("/p", "abd"));
    }
}
