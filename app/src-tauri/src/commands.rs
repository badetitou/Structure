//! Tauri commands: thin wrappers over `structure_core::pipeline`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use structure_core::matrix;
use structure_core::model::{CellSymbols, Diff, History, Impact, Matrix, Progress};
use structure_core::pipeline::{self, Analysis, Options};
use tauri::{AppHandle, Emitter, Manager};

/// One cached analysis: the working tree of a project (`fingerprint` set) or
/// the immutable snapshot of a commit (`snapshot`).
struct Entry<T> {
    key: String,
    /// Directory that was analysed (the project, or the commit's snapshot).
    dir: PathBuf,
    /// `pipeline::worktree_fingerprint` taken before the analysis.
    fingerprint: Option<String>,
    /// Id sent to the UI with the matrix (see `Cache::next_generation`).
    generation: u64,
    snapshot: bool,
    value: Arc<T>,
}

/// Snapshot analyses kept in memory (a vscode graph is ~3.5 GB): the one in
/// use plus one more. Working-tree entries are never evicted by snapshots.
const MAX_SNAPSHOT_ENTRIES: usize = 2;

/// Cached analyses, most recently used last. Working-tree entries are keyed
/// by canonical project path, commit snapshots by `<path>\0<full sha>`.
pub struct Cache<T> {
    entries: Mutex<Vec<Entry<T>>>,
    counter: AtomicU64,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Cache {
            entries: Mutex::new(Vec::new()),
            counter: AtomicU64::new(0),
        }
    }
}

pub type AnalysisCache = Cache<Analysis>;

/// Hit: directory analysed, fingerprint, generation, value.
type Hit<T> = (PathBuf, Option<String>, u64, Arc<T>);

impl<T> Cache<T> {
    fn with<R>(&self, f: impl FnOnce(&mut Vec<Entry<T>>) -> R) -> Result<R, String> {
        let mut g = self
            .entries
            .lock()
            .map_err(|_| "internal error: analysis cache poisoned".to_string())?;
        Ok(f(&mut g))
    }

    /// A fresh generation id (never 0, which means "unset").
    fn next_generation(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The entry for `key`, marked as most recently used.
    fn get(&self, key: &str) -> Result<Option<Hit<T>>, String> {
        self.with(|v| {
            let i = v.iter().position(|e| e.key == key)?;
            let e = v.remove(i);
            let out = (
                e.dir.clone(),
                e.fingerprint.clone(),
                e.generation,
                e.value.clone(),
            );
            v.push(e);
            Some(out)
        })
    }

    /// Drop the entry for `key`, e.g. before re-analysing it, so the old and
    /// the new analysis are never both held.
    fn remove(&self, key: &str) -> Result<(), String> {
        self.with(|v| v.retain(|e| e.key != key))
    }

    /// Evict the least recently used snapshot entries until at most `keep`
    /// remain. Working-tree entries are untouched.
    fn evict_snapshots(&self, keep: usize) -> Result<(), String> {
        self.with(|v| {
            let mut n = v.iter().filter(|e| e.snapshot).count();
            v.retain(|e| {
                if e.snapshot && n > keep {
                    n -= 1;
                    false
                } else {
                    true
                }
            });
        })
    }

    fn put(&self, entry: Entry<T>) -> Result<(), String> {
        self.with(|v| {
            v.retain(|e| e.key != entry.key);
            v.push(entry);
        })
    }
}

fn snapshot_key(key: &str, head: &str) -> String {
    format!("{key}\0{head}")
}

/// Error when the UI's matrix does not come from the cached analysis.
fn check_generation(cached: u64, requested: u64) -> Result<(), String> {
    if cached == requested {
        Ok(())
    } else {
        Err("analysis changed, re-run Analyze".to_string())
    }
}

const DEFAULT_PAGE: usize = 500;
const MAX_PAGE: usize = 2000;

fn page_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE).min(MAX_PAGE)
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

/// Canonical roots of a (multi-)project analysis, sorted and deduplicated, and
/// the cache key: the canonical paths joined with "\n". One path gives the
/// same key as `canonical`.
fn canonical_many(paths: &[String]) -> Result<(Vec<PathBuf>, String), String> {
    if paths.is_empty() {
        return Err("no project given".to_string());
    }
    let mut roots = paths
        .iter()
        .map(|p| canonical(p).map(|(p, _)| p))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort();
    roots.dedup();
    let key = roots
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    Ok((roots, key))
}

/// Fingerprint of a set of roots: one root's own, else every root's joined;
/// none when any root has none.
fn combined_fingerprint(roots: &[PathBuf]) -> Option<String> {
    let fps = roots
        .iter()
        .map(|r| pipeline::worktree_fingerprint(r).ok())
        .collect::<Option<Vec<_>>>()?;
    Some(fps.join("\n"))
}

/// Analyze (warnings travel in `Matrix::warnings`), emit progress, and cache
/// the result. The previous entry for `key` is dropped first.
fn run_analyze(
    app: &AppHandle,
    roots: &[PathBuf],
    key: &str,
    opts: &Options,
) -> Result<Arc<Analysis>, String> {
    let cache = app.state::<AnalysisCache>();
    cache.remove(key)?;
    // Taken before the analysis: a change made meanwhile invalidates the entry.
    let fingerprint = combined_fingerprint(roots);
    let mut a = pipeline::analyze_many(roots, opts, &|p| emit_progress_event(app, p))
        .map_err(|e| format!("{e:#}"))?;
    let generation = cache.next_generation();
    a.matrix.generation = generation;
    let a = Arc::new(a);
    cache.put(Entry {
        key: key.to_string(),
        dir: roots.first().cloned().unwrap_or_default(),
        fingerprint,
        generation,
        snapshot: false,
        value: a.clone(),
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
pub async fn analyze(app: AppHandle, paths: Vec<String>) -> Result<Matrix, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (roots, key) = canonical_many(&paths)?;
        // An explicit Analyze re-indexes when sources are newer than index.scip.
        run_analyze(&app, &roots, &key, &Options::default()).map(|a| a.matrix.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A cached snapshot analysis whose directory still exists (the snapshot
/// pruner may have removed it; that counts as a miss).
fn cached_snapshot(
    cache: &AnalysisCache,
    skey: &str,
) -> Result<Option<(PathBuf, Arc<Analysis>)>, String> {
    match cache.get(skey)? {
        Some((dir, _, _, a)) if dir.is_dir() => Ok(Some((dir, a))),
        Some(_) => {
            cache.remove(skey)?;
            Ok(None)
        }
        None => Ok(None),
    }
}

/// Impact of a commit: its snapshot is analysed once and cached under the
/// commit's full SHA, which `Impact::head` then carries.
fn impact_of_commit(
    app: &AppHandle,
    p: &Path,
    key: &str,
    base: Option<&str>,
    head: &str,
) -> Result<Impact, String> {
    let sha = pipeline::resolve_commit(p, head).map_err(|e| format!("{e:#}"))?;
    let base_sha = base
        .map(|b| pipeline::resolve_base(p, b))
        .transpose()
        .map_err(|e| format!("{e:#}"))?;
    let skey = snapshot_key(key, &sha);
    let cache = app.state::<AnalysisCache>();
    let (dir, analysis) = match cached_snapshot(&cache, &skey)? {
        Some(hit) => hit,
        None => {
            let state = app.state::<SnapshotLock>();
            let _guard = state
                .0
                .lock()
                .map_err(|_| "internal error: snapshot lock poisoned".to_string())?;
            // Another request may have analysed it while we waited.
            match cached_snapshot(&cache, &skey)? {
                Some(hit) => hit,
                None => {
                    emit_progress(app, &format!("checking out {}", short_rev(&sha)), 2);
                    // Make room first: never hold the old and the new graph.
                    cache.evict_snapshots(MAX_SNAPSHOT_ENTRIES - 1)?;
                    let (dir, mut a) =
                        pipeline::analyze_snapshot(p, &sha, &Options::default(), &|pr| {
                            emit_progress_event(app, pr)
                        })
                        .map_err(|e| format!("{e:#}"))?;
                    let generation = cache.next_generation();
                    a.matrix.generation = generation;
                    let a = Arc::new(a);
                    cache.put(Entry {
                        key: skey,
                        dir: dir.clone(),
                        fingerprint: None,
                        generation,
                        snapshot: true,
                        value: a.clone(),
                    })?;
                    (dir, a)
                }
            }
        }
    };
    pipeline::impact_in_snapshot(&dir, &analysis, base_sha.as_deref(), &sha)
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
                .state::<AnalysisCache>()
                .get(&key)?
                .and_then(|(_, fp, _, a)| (fp.as_deref() == Some(cur.as_str())).then_some(a)),
            None => None,
        };
        let analysis = match hit {
            Some(a) => a,
            None => {
                let opts = Options {
                    reindex_if_stale: false,
                    ..Options::default()
                };
                run_analyze(&app, std::slice::from_ref(&p), &key, &opts)?
            }
        };
        emit_progress(&app, "computing impact", 50);
        let result =
            pipeline::impact(&p, &analysis, base.as_deref(), None).map_err(|e| format!("{e:#}"))?;
        emit_progress(&app, "done", 100);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Symbols of the given cells (the matrix carries counts only), paged.
/// `generation` is `Matrix::generation` of the matrix on screen; `head` is
/// the full SHA from `Impact::head`, or null for the working tree.
#[tauri::command]
pub async fn cell_symbols(
    app: AppHandle,
    paths: Vec<String>,
    head: Option<String>,
    generation: u64,
    cells: Vec<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<CellSymbols, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, key) = canonical_many(&paths)?;
        let key = match head.as_deref() {
            Some(h) => snapshot_key(&key, h),
            None => key,
        };
        let (_, _, cached_gen, a) = app
            .state::<AnalysisCache>()
            .get(&key)?
            .ok_or_else(|| "analysis changed, re-run Analyze".to_string())?;
        check_generation(cached_gen, generation)?;
        Ok(matrix::cell_symbols(
            &a.graph,
            &a.classification,
            &cells,
            offset.unwrap_or(0),
            page_limit(limit),
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

/// Textual diff of `base..head` (same meaning as `impact`), bounded in size.
#[tauri::command]
pub async fn diff(
    path: String,
    base: Option<String>,
    head: Option<String>,
) -> Result<Diff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, _) = canonical(&path)?;
        structure_core::diff::diff(&p, base.as_deref(), head.as_deref())
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, snapshot: bool, generation: u64) -> Entry<u32> {
        Entry {
            key: key.to_string(),
            dir: PathBuf::from("/x"),
            fingerprint: None,
            generation,
            snapshot,
            value: Arc::new(generation as u32),
        }
    }

    fn keys(c: &Cache<u32>) -> Vec<String> {
        c.with(|v| v.iter().map(|e| e.key.clone()).collect())
            .unwrap()
    }

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
    fn canonical_many_sorts_dedups_and_keeps_single_key() {
        let tmp = std::env::temp_dir();
        let t = tmp.to_string_lossy().into_owned();
        let (_, one) = canonical(&t).unwrap();
        let (roots, key) = canonical_many(std::slice::from_ref(&t)).unwrap();
        assert_eq!((roots.len(), &key), (1, &one));
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let (_, a) = canonical_many(&[t.clone(), cwd.clone()]).unwrap();
        let (_, b) = canonical_many(&[cwd.clone(), t.clone(), t]).unwrap();
        assert_eq!(a, b);
        assert!(a.contains('\n'));
        assert!(canonical_many(&[]).is_err());
        assert!(canonical_many(&["/definitely/not/here".to_string()]).is_err());
    }

    #[test]
    fn snapshot_keys_differ_from_worktree_key() {
        assert_ne!(snapshot_key("/p", "abc"), "/p");
        assert_ne!(snapshot_key("/p", "abc"), snapshot_key("/p", "abd"));
    }

    #[test]
    fn snapshots_never_evict_working_tree_entries() {
        let c = Cache::<u32>::default();
        c.put(entry("/p", false, 1)).unwrap();
        c.put(entry("/q", false, 2)).unwrap();
        for (i, sha) in ["a", "b", "c", "d"].iter().enumerate() {
            c.evict_snapshots(MAX_SNAPSHOT_ENTRIES - 1).unwrap();
            c.put(entry(&snapshot_key("/p", sha), true, 10 + i as u64))
                .unwrap();
        }
        let k = keys(&c);
        assert!(k.contains(&"/p".to_string()) && k.contains(&"/q".to_string()));
        let snaps = k.iter().filter(|k| k.contains('\0')).count();
        assert_eq!(snaps, MAX_SNAPSHOT_ENTRIES);
        // Least recently used snapshot went first; the newest two remain.
        assert!(k.contains(&snapshot_key("/p", "d")) && k.contains(&snapshot_key("/p", "c")));
    }

    #[test]
    fn get_refreshes_recency_and_remove_drops_entry() {
        let c = Cache::<u32>::default();
        c.put(entry(&snapshot_key("/p", "a"), true, 1)).unwrap();
        c.put(entry(&snapshot_key("/p", "b"), true, 2)).unwrap();
        assert!(c.get(&snapshot_key("/p", "a")).unwrap().is_some());
        c.evict_snapshots(1).unwrap();
        assert_eq!(keys(&c), vec![snapshot_key("/p", "a")]);
        c.remove(&snapshot_key("/p", "a")).unwrap();
        assert!(c.get(&snapshot_key("/p", "a")).unwrap().is_none());
    }

    #[test]
    fn generations_are_unique_and_checked() {
        let c = Cache::<u32>::default();
        let (a, b) = (c.next_generation(), c.next_generation());
        assert!(a > 0 && b > a);
        assert!(check_generation(a, a).is_ok());
        let e = check_generation(b, a).unwrap_err();
        assert!(e.contains("re-run Analyze"));
    }

    #[test]
    fn page_limit_is_clamped() {
        assert_eq!(page_limit(None), DEFAULT_PAGE);
        assert_eq!(page_limit(Some(10)), 10);
        assert_eq!(page_limit(Some(1_000_000)), MAX_PAGE);
    }
}
