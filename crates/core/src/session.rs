//! A `Session` owns the analyses a front end works on: it caches them (working
//! trees by canonical path and fingerprint, commit snapshots by full SHA),
//! hands out generation ids with each matrix, and answers the follow-up
//! requests (cell symbols, impact, history, diff) against the cached ones.
//!
//! It is `Send + Sync` with interior locking, so one instance can be shared by
//! concurrent requests (e.g. Tauri managed state). Errors are `anyhow`; a
//! front end shows them with `{e:#}`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail};

use crate::matrix;
use crate::model::{CellSymbols, Diff, History, Impact, Matrix, Progress};
use crate::pipeline::{self, Analysis, Options};

/// One cached analysis: the working tree of a project (`fingerprint` set) or
/// the immutable snapshot of a commit (`snapshot`).
struct Entry<T> {
    key: String,
    /// Directory that was analysed (the project, or the commit's snapshot).
    /// For a multi-root working-tree entry it is the first (sorted) root;
    /// nothing reads it for working-tree entries (only snapshot entries use it).
    dir: PathBuf,
    /// `pipeline::worktree_fingerprint` taken before the analysis.
    fingerprint: Option<String>,
    /// Id sent with the matrix (see `Cache::next_generation`).
    generation: u64,
    snapshot: bool,
    value: Arc<T>,
}

/// Snapshot analyses kept in memory (a vscode graph is ~3.5 GB): the one in
/// use plus one more. Working-tree entries are never evicted by snapshots.
const MAX_SNAPSHOT_ENTRIES: usize = 2;

/// Cached analyses, most recently used last. Working-tree entries are keyed
/// by canonical project path, commit snapshots by `<path>\0<full sha>`.
struct Cache<T> {
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

/// Hit: directory analysed, fingerprint, generation, value.
type Hit<T> = (PathBuf, Option<String>, u64, Arc<T>);

impl<T> Cache<T> {
    fn with<R>(&self, f: impl FnOnce(&mut Vec<Entry<T>>) -> R) -> anyhow::Result<R> {
        let mut g = self
            .entries
            .lock()
            .map_err(|_| anyhow!("internal error: analysis cache poisoned"))?;
        Ok(f(&mut g))
    }

    /// A fresh generation id (never 0, which means "unset").
    fn next_generation(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The entry for `key`, marked as most recently used.
    fn get(&self, key: &str) -> anyhow::Result<Option<Hit<T>>> {
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
    fn remove(&self, key: &str) -> anyhow::Result<()> {
        self.with(|v| v.retain(|e| e.key != key))
    }

    /// Evict the least recently used snapshot entries until at most `keep`
    /// remain. Working-tree entries are untouched.
    fn evict_snapshots(&self, keep: usize) -> anyhow::Result<()> {
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

    fn put(&self, entry: Entry<T>) -> anyhow::Result<()> {
        self.with(|v| {
            v.retain(|e| e.key != entry.key);
            v.push(entry);
        })
    }
}

fn snapshot_key(key: &str, head: &str) -> String {
    format!("{key}\0{head}")
}

/// Error when the caller's matrix does not come from the cached analysis.
fn check_generation(cached: u64, requested: u64) -> anyhow::Result<()> {
    if cached == requested {
        Ok(())
    } else {
        bail!("analysis changed, re-run Analyze")
    }
}

const DEFAULT_PAGE: usize = 500;
const MAX_PAGE: usize = 2000;

fn page_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE).min(MAX_PAGE)
}

/// Commits listed by [`Session::history`] when no limit is given.
const DEFAULT_HISTORY: u32 = 200;

fn step(progress: &dyn Fn(Progress), step: &str, pct: u8) {
    progress(Progress {
        step: step.to_string(),
        pct,
        detail: None,
    });
}

fn canonical(path: &str) -> anyhow::Result<(PathBuf, String)> {
    let p = Path::new(path)
        .canonicalize()
        .map_err(|e| anyhow!("cannot open {path}: {e}"))?;
    let key = p.to_string_lossy().into_owned();
    Ok((p, key))
}

/// Canonical roots of a (multi-)project analysis, sorted and deduplicated, and
/// the cache key: the canonical paths joined with "\n". One path gives the
/// same key as `canonical`.
fn canonical_many(paths: &[String]) -> anyhow::Result<(Vec<PathBuf>, String)> {
    if paths.is_empty() {
        bail!("no project given");
    }
    let mut roots = paths
        .iter()
        .map(|p| canonical(p).map(|(p, _)| p))
        .collect::<anyhow::Result<Vec<_>>>()?;
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
    combine_fingerprints(
        roots
            .iter()
            .map(|r| pipeline::worktree_fingerprint(r).ok())
            .collect(),
    )
}

/// One fingerprint is kept as is, several are joined; none when any is missing.
fn combine_fingerprints(fps: Vec<Option<String>>) -> Option<String> {
    let fps = fps.into_iter().collect::<Option<Vec<_>>>()?;
    Some(fps.join("\n"))
}

/// Display form of a revision: 7 chars for a full hex SHA, else as given.
fn short_rev(rev: &str) -> &str {
    if rev.len() >= 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
        &rev[..7]
    } else {
        rev
    }
}

/// Cached analyses and the requests over them. See the module docs.
pub struct Session {
    cache: Cache<Analysis>,
    /// Serialises snapshot analyses: concurrent checkouts/indexing of the
    /// same snapshot would race.
    snapshot_lock: Mutex<()>,
    /// Options of the analyses [`Session::impact`] runs itself.
    impact_options: Options,
}

impl Default for Session {
    fn default() -> Self {
        Session::with_impact_options(Options::default())
    }
}

impl Session {
    pub fn new() -> Self {
        Session::default()
    }

    /// A session whose impact analyses (snapshots, and the working tree when
    /// it changed) use `opts`; the working tree still never re-indexes a
    /// stale index there.
    pub fn with_impact_options(opts: Options) -> Self {
        Session {
            cache: Cache::default(),
            snapshot_lock: Mutex::new(()),
            impact_options: opts,
        }
    }

    /// Analyze the projects at `paths` (warnings travel in
    /// `Matrix::warnings`) and cache the result; `Matrix::generation` then
    /// identifies it for [`Session::cell_symbols`].
    pub fn analyze(
        &self,
        paths: &[String],
        opts: &Options,
        progress: &dyn Fn(Progress),
    ) -> anyhow::Result<Matrix> {
        let (roots, key) = canonical_many(paths)?;
        self.run_analyze(&roots, &key, opts, progress)
            .map(|a| a.matrix.clone())
    }

    /// Analyze and cache the result. The previous entry for `key` is dropped
    /// first.
    fn run_analyze(
        &self,
        roots: &[PathBuf],
        key: &str,
        opts: &Options,
        progress: &dyn Fn(Progress),
    ) -> anyhow::Result<Arc<Analysis>> {
        self.cache.remove(key)?;
        // Taken before the analysis: a change made meanwhile invalidates the entry.
        let fingerprint = combined_fingerprint(roots);
        let mut a = pipeline::analyze_many(roots, opts, progress)?;
        let generation = self.cache.next_generation();
        a.matrix.generation = generation;
        let a = Arc::new(a);
        self.cache.put(Entry {
            key: key.to_string(),
            dir: roots.first().cloned().unwrap_or_default(),
            fingerprint,
            generation,
            snapshot: false,
            value: a.clone(),
        })?;
        Ok(a)
    }

    /// Impact of `base..head` on the project at `path`. Without `head` it is
    /// the working tree: its cached analysis is reused while nothing changed
    /// since (same HEAD, status and file mtimes), else it is re-parsed so
    /// symbol ranges match the diff (the possibly stale index is reused).
    /// With `head`, the commit's snapshot is analysed once and cached under
    /// its full SHA, which `Impact::head` then carries.
    pub fn impact(
        &self,
        path: &str,
        base: Option<&str>,
        head: Option<&str>,
        progress: &dyn Fn(Progress),
    ) -> anyhow::Result<Impact> {
        let (p, key) = canonical(path)?;
        if let Some(h) = head {
            let result = self.impact_of_commit(&p, &key, base, h, progress)?;
            step(progress, "done", 100);
            return Ok(result);
        }
        let current = pipeline::worktree_fingerprint(&p).ok();
        let hit = match &current {
            Some(cur) => self
                .cache
                .get(&key)?
                .and_then(|(_, fp, _, a)| (fp.as_deref() == Some(cur.as_str())).then_some(a)),
            None => None,
        };
        let analysis = match hit {
            Some(a) => a,
            None => {
                let opts = Options {
                    reindex_if_stale: false,
                    ..self.impact_options
                };
                self.run_analyze(std::slice::from_ref(&p), &key, &opts, progress)?
            }
        };
        step(progress, "computing impact", 50);
        let result = pipeline::impact(&p, &analysis, base, None)?;
        step(progress, "done", 100);
        Ok(result)
    }

    /// A cached snapshot analysis whose directory still exists (the snapshot
    /// pruner may have removed it; that counts as a miss).
    fn cached_snapshot(&self, skey: &str) -> anyhow::Result<Option<(PathBuf, Arc<Analysis>)>> {
        match self.cache.get(skey)? {
            Some((dir, _, _, a)) if dir.is_dir() => Ok(Some((dir, a))),
            Some(_) => {
                self.cache.remove(skey)?;
                Ok(None)
            }
            None => Ok(None),
        }
    }

    fn impact_of_commit(
        &self,
        p: &Path,
        key: &str,
        base: Option<&str>,
        head: &str,
        progress: &dyn Fn(Progress),
    ) -> anyhow::Result<Impact> {
        let sha = pipeline::resolve_commit(p, head)?;
        let base_sha = base.map(|b| pipeline::resolve_base(p, b)).transpose()?;
        let skey = snapshot_key(key, &sha);
        let (dir, analysis) = match self.cached_snapshot(&skey)? {
            Some(hit) => hit,
            None => {
                let _guard = self
                    .snapshot_lock
                    .lock()
                    .map_err(|_| anyhow!("internal error: snapshot lock poisoned"))?;
                // Another request may have analysed it while we waited.
                match self.cached_snapshot(&skey)? {
                    Some(hit) => hit,
                    None => {
                        step(progress, &format!("checking out {}", short_rev(&sha)), 2);
                        // Make room first: never hold the old and the new graph.
                        self.cache.evict_snapshots(MAX_SNAPSHOT_ENTRIES - 1)?;
                        let (dir, mut a) =
                            pipeline::analyze_snapshot(p, &sha, &self.impact_options, progress)?;
                        let generation = self.cache.next_generation();
                        a.matrix.generation = generation;
                        let a = Arc::new(a);
                        self.cache.put(Entry {
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
    }

    /// Symbols of the given cells (the matrix carries counts only), paged.
    /// `generation` is `Matrix::generation` of the matrix the caller shows;
    /// `head` is the full SHA from `Impact::head`, or `None` for the working
    /// tree.
    pub fn cell_symbols(
        &self,
        paths: &[String],
        head: Option<&str>,
        generation: u64,
        cells: &[String],
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> anyhow::Result<CellSymbols> {
        let (_, key) = canonical_many(paths)?;
        let key = match head {
            Some(h) => snapshot_key(&key, h),
            None => key,
        };
        let (_, _, cached_gen, a) = self
            .cache
            .get(&key)?
            .ok_or_else(|| anyhow!("analysis changed, re-run Analyze"))?;
        check_generation(cached_gen, generation)?;
        Ok(matrix::cell_symbols(
            &a.graph,
            &a.classification,
            cells,
            offset.unwrap_or(0),
            page_limit(limit),
        ))
    }

    /// Commits of the project at `path`, newest first (200 by default).
    pub fn history(&self, path: &str, limit: Option<u32>) -> anyhow::Result<History> {
        let (p, _) = canonical(path)?;
        pipeline::history(&p, limit.unwrap_or(DEFAULT_HISTORY))
    }

    /// Textual diff of `base..head` (same meaning as [`Session::impact`]),
    /// bounded in size.
    pub fn diff(&self, path: &str, base: Option<&str>, head: Option<&str>) -> anyhow::Result<Diff> {
        let (p, _) = canonical(path)?;
        crate::diff::diff(&p, base, head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_support::{no_scip, temp_repo};

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
    fn session_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Session>();
    }

    #[test]
    fn short_rev_shortens_only_full_shas() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(short_rev(sha), "0123456");
        assert_eq!(short_rev("main"), "main");
    }

    #[test]
    fn canonical_reports_missing_path() {
        let e = canonical("/definitely/not/here").unwrap_err();
        assert!(
            format!("{e:#}").starts_with("cannot open /definitely/not/here: "),
            "{e:#}"
        );
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
    fn combine_fingerprints_keeps_one_joins_many_and_needs_all() {
        let f = |s: &str| Some(s.to_string());
        assert_eq!(combine_fingerprints(vec![f("a")]), f("a"));
        assert_eq!(combine_fingerprints(vec![f("a"), f("b")]), f("a\nb"));
        assert_eq!(combine_fingerprints(vec![f("a"), None]), None);
        assert_eq!(combine_fingerprints(vec![]), f(""));
        // Not a repository: no fingerprint, so a single root has none either.
        let none = PathBuf::from("/definitely/not/here");
        assert_eq!(combined_fingerprint(&[none]), None);
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
        assert!(e.to_string().contains("re-run Analyze"));
    }

    #[test]
    fn page_limit_is_clamped() {
        assert_eq!(page_limit(None), DEFAULT_PAGE);
        assert_eq!(page_limit(Some(10)), 10);
        assert_eq!(page_limit(Some(1_000_000)), MAX_PAGE);
    }

    #[test]
    fn session_api_over_a_temp_repo() {
        let (repo, c1, c2) = temp_repo("session");
        let root = repo.join("app").to_string_lossy().into_owned();
        let paths = vec![root.clone()];
        let s = Session::with_impact_options(no_scip());
        let steps = Mutex::new(Vec::<String>::new());
        let record = |p: Progress| steps.lock().unwrap().push(p.step);

        // Analyze: a fresh generation each time; symbols need the current one.
        let m = s.analyze(&paths, &no_scip(), &record).unwrap();
        assert!(m.generation > 0 && !m.cells.is_empty());
        let cells: Vec<String> = m.cells.iter().map(|c| c.id.clone()).collect();
        let syms = s
            .cell_symbols(&paths, None, m.generation, &cells, None, None)
            .unwrap();
        assert!(syms.total >= 2, "A and B (and their methods)");
        let m2 = s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        assert!(m2.generation > m.generation);
        let e = s
            .cell_symbols(&paths, None, m.generation, &cells, None, None)
            .unwrap_err();
        assert_eq!(format!("{e:#}"), "analysis changed, re-run Analyze");

        // Working-tree impact reuses the cached analysis while nothing changed.
        steps.lock().unwrap().clear();
        let i = s.impact(&root, Some(&c1), None, &record).unwrap();
        assert!(!i.changed_files.is_empty(), "c2 changed A.java");
        assert_eq!(
            *steps.lock().unwrap(),
            vec!["computing impact".to_string(), "done".to_string()]
        );
        assert!(
            s.cell_symbols(&paths, None, m2.generation, &cells, None, None)
                .is_ok(),
            "unchanged tree keeps the analysis"
        );
        // A change re-analyses it (new generation).
        std::fs::write(repo.join("app/src/C.java"), "class C {}\n").unwrap();
        s.impact(&root, Some(&c1), None, &|_| {}).unwrap();
        assert!(
            s.cell_symbols(&paths, None, m2.generation, &cells, None, None)
                .is_err()
        );

        // Commit impact: snapshot analysed once, then reused.
        steps.lock().unwrap().clear();
        let ci = s.impact(&root, Some(&c1), Some(&c2), &record).unwrap();
        assert_eq!(ci.head.as_deref(), Some(c2.as_str()));
        let first = steps.lock().unwrap().clone();
        assert_eq!(
            first.first().map(String::as_str),
            Some(&*format!("checking out {}", &c2[..7]))
        );
        assert_eq!(first.last().map(String::as_str), Some("done"));
        steps.lock().unwrap().clear();
        s.impact(&root, Some(&c1), Some(&c2), &record).unwrap();
        assert_eq!(*steps.lock().unwrap(), vec!["done".to_string()]);
        // Snapshot cells are found under the snapshot key only.
        let e = s
            .cell_symbols(&paths, Some(&c1), 1, &cells, None, None)
            .unwrap_err();
        assert!(format!("{e:#}").contains("re-run Analyze"));

        // History and diff.
        let h = s.history(&root, Some(2)).unwrap();
        assert_eq!(h.commits.len(), 2);
        let d = s.diff(&root, Some(&c1), Some(&c2)).unwrap();
        assert!(!d.files.is_empty());
        let e = s.history("/definitely/not/here", None).unwrap_err();
        assert!(format!("{e:#}").starts_with("cannot open /definitely/not/here: "));
        assert_eq!(
            format!("{:#}", s.analyze(&[], &no_scip(), &|_| {}).unwrap_err()),
            "no project given"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}
