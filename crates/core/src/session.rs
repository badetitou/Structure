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

use crate::lens::Lens;
use crate::matrix;
use crate::model::{
    CellSymbols, Diff, History, Impact, ImpactedSymbol, LensCells, LensInfo, Matrix, Progress,
};
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

    /// Drop the commit snapshots of the project keyed `key`.
    fn remove_snapshots_of(&self, key: &str) -> anyhow::Result<()> {
        let prefix = snapshot_key(key, "");
        self.with(|v| v.retain(|e| !(e.snapshot && e.key.starts_with(&prefix))))
    }

    /// Swap the value of `key`'s entry, if it still has generation `expected`,
    /// for `value` under the fresh `generation` and `fingerprint`. False when
    /// the entry is gone or was replaced meanwhile.
    fn replace(
        &self,
        key: &str,
        expected: u64,
        generation: u64,
        fingerprint: Option<String>,
        value: Arc<T>,
    ) -> anyhow::Result<bool> {
        self.with(|v| {
            let Some(e) = v
                .iter_mut()
                .find(|e| e.key == key && e.generation == expected)
            else {
                return false;
            };
            e.generation = generation;
            e.fingerprint = fingerprint;
            e.value = value;
            true
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

/// A fingerprint without the lines of project rules files
/// ([`pipeline::PROJECT_RULES`]): equal for two working trees that differ in
/// their rules only.
fn without_rules(fp: &str) -> String {
    let suffix = format!("/{}", pipeline::PROJECT_RULES);
    fp.lines()
        .filter(|l| {
            // Status lines read `XY <path>\t<len>\t<mtime>`.
            let path = l.get(3..).and_then(|r| r.split('\t').next());
            !path.is_some_and(|p| p == pipeline::PROJECT_RULES || p.ends_with(&suffix))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Fingerprint to keep for a working-tree entry once reclassified: the
/// current one when the tree only changed in its rules files since `cached`
/// was taken (the rules are now applied), else `cached` unchanged.
fn reclassified_fingerprint(cached: Option<String>, current: Option<String>) -> Option<String> {
    match (cached, current) {
        (Some(old), Some(cur)) if without_rules(&old) == without_rules(&cur) => Some(cur),
        (old, _) => old,
    }
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

    /// Re-apply the rules of the projects at `paths` (as given to
    /// [`Session::analyze`]) to their cached analysis without re-indexing or
    /// re-parsing: classification, layers and matrix are recomputed from the
    /// cached graph, which is kept along with its impact index. The new
    /// matrix carries a fresh generation, so requests made against the old
    /// one fail with "analysis changed". A rules file is the only change
    /// that keeps the working tree fresh for [`Session::impact`].
    ///
    /// The cached commit snapshots of each project root are dropped: they were
    /// classified with the rules of their own commit, and dropping them keeps
    /// every cached classification one a fresh analysis would give (the next
    /// commit impact analyses its snapshot again). On error (no cached
    /// analysis, invalid rules) the cache is unchanged.
    pub fn reclassify(&self, paths: &[String]) -> anyhow::Result<Matrix> {
        self.reclassify_then(paths, &|| {})
    }

    /// [`Session::reclassify`], calling `after_rules` once the rules are read
    /// (tests use it to edit them in that window).
    fn reclassify_then(&self, paths: &[String], after_rules: &dyn Fn()) -> anyhow::Result<Matrix> {
        let (roots, key) = canonical_many(paths)?;
        let (_, fingerprint, generation, cached) = self
            .cache
            .get(&key)?
            .ok_or_else(|| anyhow!("no analysis to reclassify, run Analyze"))?;
        // Taken before the rules are read: a rules edit made meanwhile leaves
        // the entry stale rather than marking it applied.
        let current = combined_fingerprint(&roots);
        let mut a = pipeline::reclassify(&cached)?;
        after_rules();
        drop(cached);
        let fingerprint = reclassified_fingerprint(fingerprint, current);
        let next = self.cache.next_generation();
        a.matrix.generation = next;
        let matrix = a.matrix.clone();
        if !self
            .cache
            .replace(&key, generation, next, fingerprint, Arc::new(a))?
        {
            bail!("analysis changed, re-run Analyze");
        }
        // Snapshots are keyed per single root (see `Session::impact`).
        for root in &roots {
            self.cache.remove_snapshots_of(&root.to_string_lossy())?;
        }
        Ok(matrix)
    }

    /// Stamp of the rules files of the projects at `paths` (see
    /// [`pipeline::rules_stamp`]): poll it and call [`Session::reclassify`]
    /// when it changes. The cached analysis' roots are reused so a poll only
    /// stats the rules files; without one the roots are expanded again.
    pub fn rules_stamp(&self, paths: &[String]) -> anyhow::Result<String> {
        let (roots, key) = canonical_many(paths)?;
        match self.cache.get(&key)? {
            Some((_, _, _, a)) => Ok(pipeline::rules_stamp_of(&a.roots)),
            None => pipeline::rules_stamp(&roots),
        }
    }

    /// Impact of `base..head` on the project at `path`. Without `head` it is
    /// the working tree: its cached analysis is reused while nothing changed
    /// since (same HEAD, status and file mtimes), else it is re-parsed so
    /// symbol ranges match the diff (the possibly stale index is reused);
    /// invalid project rules are then an error that keeps the cached analysis.
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
                let roots = std::slice::from_ref(&p);
                // Fail before the cached analysis is dropped: invalid rules
                // keep it, as [`Session::reclassify`] does.
                pipeline::check_rules(roots)?;
                self.run_analyze(roots, &key, &opts, progress)?
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

    /// The cached analysis a caller's matrix comes from: of the projects at
    /// `paths` (working tree), or of commit `head` (the full SHA from
    /// `Impact::head`); `generation` is `Matrix::generation` of that matrix.
    fn cached(
        &self,
        paths: &[String],
        head: Option<&str>,
        generation: u64,
    ) -> anyhow::Result<Arc<Analysis>> {
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
        Ok(a)
    }

    /// The lenses a matrix can be requested in, the default first.
    pub fn lenses() -> Vec<LensInfo> {
        crate::lens::all()
    }

    /// The matrix of a cached analysis (same `paths`, `head` and `generation`
    /// as [`Session::cell_symbols`]) through lens `lens` (an id of
    /// [`Session::lenses`]). It carries the analysis' generation; the default
    /// lens gives the matrix [`Session::analyze`] returned. Another lens is
    /// built once per analysis, then cached with it.
    pub fn matrix(
        &self,
        paths: &[String],
        head: Option<&str>,
        generation: u64,
        lens: &str,
    ) -> anyhow::Result<Matrix> {
        let lens = Lens::parse(lens)?;
        let a = self.cached(paths, head, generation)?;
        Ok(a.lens(lens)?.matrix().clone())
    }

    /// Symbols of the given cells (the matrix carries counts only), paged.
    /// `generation` is `Matrix::generation` of the matrix the caller shows;
    /// `head` is the full SHA from `Impact::head`, or `None` for the working
    /// tree; `lens` the lens of the cell ids (`None`: the default lens).
    #[allow(clippy::too_many_arguments)]
    pub fn cell_symbols(
        &self,
        paths: &[String],
        head: Option<&str>,
        generation: u64,
        lens: Option<&str>,
        cells: &[String],
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> anyhow::Result<CellSymbols> {
        let lens = Lens::parse_or_default(lens)?;
        let a = self.cached(paths, head, generation)?;
        let view = a.lens(lens)?;
        Ok(matrix::cell_symbols_in(
            &a.graph,
            view.classification(),
            lens.cell_prefix(),
            cells,
            offset.unwrap_or(0),
            page_limit(limit),
        ))
    }

    /// The cells of an impact of the project at `path` in lens `lens`, from
    /// its `changed` and `impacted` symbols (`Impact::changed_symbols` and
    /// `impacted_symbols`); `head` is `Impact::head`. The symbols are mapped
    /// with the analysis cached now for that working tree or commit (no
    /// generation: an impact carries none). The default lens gives the
    /// impact's own cells.
    pub fn impact_cells(
        &self,
        path: &str,
        head: Option<&str>,
        lens: &str,
        changed: &[String],
        impacted: &[ImpactedSymbol],
    ) -> anyhow::Result<LensCells> {
        let lens = Lens::parse(lens)?;
        let (_, key) = canonical(path)?;
        let key = match head {
            Some(h) => snapshot_key(&key, h),
            None => key,
        };
        let (_, _, _, a) = self
            .cache
            .get(&key)?
            .ok_or_else(|| anyhow!("analysis changed, re-run Analyze"))?;
        let view = a.lens(lens)?;
        Ok(pipeline::lens_cells(&view, changed, impacted))
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
            .cell_symbols(&paths, None, m.generation, None, &cells, None, None)
            .unwrap();
        assert!(syms.total >= 2, "A and B (and their methods)");
        let m2 = s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        assert!(m2.generation > m.generation);
        let e = s
            .cell_symbols(&paths, None, m.generation, None, &cells, None, None)
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
            s.cell_symbols(&paths, None, m2.generation, None, &cells, None, None)
                .is_ok(),
            "unchanged tree keeps the analysis"
        );
        // A change re-analyses it (new generation).
        std::fs::write(repo.join("app/src/C.java"), "class C {}\n").unwrap();
        s.impact(&root, Some(&c1), None, &|_| {}).unwrap();
        assert!(
            s.cell_symbols(&paths, None, m2.generation, None, &cells, None, None)
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
            .cell_symbols(&paths, Some(&c1), 1, None, &cells, None, None)
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

    #[test]
    fn lenses_matrices_symbols_and_impact_cells() {
        use crate::pipeline::test_support::git_ok;
        let (repo, c1, _) = temp_repo("lens");
        let root = repo.join("app");
        let dir = root.join("src/com/shop/account");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Acc.java"),
            "package com.shop.account;\nclass Acc { void m() {} }\n",
        )
        .unwrap();
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "c3"]);
        let path = root.to_string_lossy().into_owned();
        let paths = vec![path.clone()];
        let s = Session::with_impact_options(no_scip());
        let ids: Vec<String> = Session::lenses().into_iter().map(|l| l.id).collect();
        assert_eq!(ids, ["layer-domain", "layer-package"]);

        let m = s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        let g = m.generation;
        // The default lens is the analysed matrix itself.
        assert_eq!(s.matrix(&paths, None, g, "layer-domain").unwrap(), m);
        let p = s.matrix(&paths, None, g, "layer-package").unwrap();
        assert_eq!(
            (p.generation, &p.layers, &p.stats),
            (g, &m.layers, &m.stats)
        );
        assert!(
            p.cells.iter().all(|c| c.id.starts_with("package/")),
            "{p:?}"
        );
        let mut domains = p.domains.clone();
        domains.sort();
        assert_eq!(domains, ["(default)", "com.shop.account"]);
        assert!(p.domain_tree.is_some());
        let size = |m: &Matrix| m.cells.iter().map(|c| c.size).sum::<usize>();
        assert_eq!(size(&p), size(&m));

        // Built once per analysis, then cached with it.
        let a = s.cached(&paths, None, g).unwrap();
        assert_eq!(a.lenses_built(), 1);
        assert_eq!(s.matrix(&paths, None, g, "layer-package").unwrap(), p);
        assert_eq!(a.lenses_built(), 1);
        let e = s.matrix(&paths, None, g, "nope").unwrap_err();
        assert_eq!(format!("{e:#}"), "unknown lens nope");
        assert!(s.matrix(&paths, None, g + 100, "layer-package").is_err());

        // Symbols of a lens' cells.
        let pcells: Vec<String> = p.cells.iter().map(|c| c.id.clone()).collect();
        let acc: Vec<String> = p
            .cells
            .iter()
            .filter(|c| c.domain == "com.shop.account")
            .map(|c| c.id.clone())
            .collect();
        let syms = s
            .cell_symbols(&paths, None, g, Some("layer-package"), &acc, None, None)
            .unwrap();
        let names: Vec<&str> = syms.symbols.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Acc"]);
        let all = s
            .cell_symbols(&paths, None, g, Some("layer-package"), &pcells, None, None)
            .unwrap();
        assert_eq!(all.total, size(&m));
        // Ids of one lens mean nothing in another.
        let none = s
            .cell_symbols(&paths, None, g, None, &pcells, None, None)
            .unwrap();
        assert_eq!(none.total, 0);

        // Impact cells per lens: the default lens gives the impact's own.
        let i = s.impact(&path, Some(&c1), None, &|_| {}).unwrap();
        let own = s
            .impact_cells(
                &path,
                None,
                "layer-domain",
                &i.changed_symbols,
                &i.impacted_symbols,
            )
            .unwrap();
        assert_eq!(own.changed_cells, i.changed_cells);
        assert_eq!(own.impacted_cells, i.impacted_cells);
        let pk = s
            .impact_cells(
                &path,
                None,
                "layer-package",
                &i.changed_symbols,
                &i.impacted_symbols,
            )
            .unwrap();
        assert!(!pk.changed_cells.is_empty());
        assert!(
            pk.changed_cells.iter().all(|c| pcells.contains(c)),
            "{pk:?}"
        );
        assert!(
            pk.changed_cells
                .iter()
                .any(|c| c.ends_with(":com.shop.account"))
        );

        // Reclassify: a fresh analysis, its lenses built again.
        let r = s.reclassify(&paths).unwrap();
        assert_eq!(
            s.cached(&paths, None, r.generation).unwrap().lenses_built(),
            0
        );
        assert_eq!(
            s.matrix(&paths, None, r.generation, "layer-package")
                .unwrap()
                .cells,
            p.cells
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    const VIEW_RULES: &str = "[[layer]]\nid = \"view\"\nlabel = \"Views\"\norder = 0\n\
                              [[rule]]\nlayer = \"view\"\nname_suffix = [\"A\"]\n";

    fn layer_ids(m: &Matrix) -> Vec<String> {
        m.layers.iter().map(|l| l.id.to_string()).collect()
    }

    #[test]
    fn without_rules_ignores_only_rules_files() {
        let fp = "sha\n?? app/.structure/rules.toml\t10\t5\n M app/src/A.java\t3\t4";
        let edited = "sha\n?? app/.structure/rules.toml\t12\t9\n M app/src/A.java\t3\t4";
        assert_eq!(without_rules(fp), without_rules(edited));
        assert_eq!(without_rules(fp), "sha\n M app/src/A.java\t3\t4");
        assert_eq!(without_rules("sha\n?? .structure/rules.toml\t1\t1"), "sha");
        let source = "sha\n?? app/.structure/rules.toml\t10\t5\n M app/src/A.java\t3\t7";
        assert_ne!(without_rules(fp), without_rules(source));
        let s = |x: &str| Some(x.to_string());
        assert_eq!(reclassified_fingerprint(s(fp), s(edited)), s(edited));
        assert_eq!(reclassified_fingerprint(s(fp), s(source)), s(fp));
        assert_eq!(reclassified_fingerprint(s(fp), None), s(fp));
        assert_eq!(reclassified_fingerprint(None, s(edited)), None);
    }

    #[test]
    fn reclassify_applies_new_rules_without_parsing_again() {
        let (repo, _, _) = temp_repo("reclassify");
        let root = repo.join("app");
        let paths = vec![root.to_string_lossy().into_owned()];
        let s = Session::with_impact_options(no_scip());
        let m = s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        assert!(!layer_ids(&m).contains(&"view".to_string()));
        let old_cells: Vec<String> = m.cells.iter().map(|c| c.id.clone()).collect();

        // Sources gone: only the cached graph can produce A and B now.
        std::fs::remove_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".structure")).unwrap();
        std::fs::write(root.join(pipeline::PROJECT_RULES), VIEW_RULES).unwrap();
        let r = s.reclassify(&paths).unwrap();
        assert_eq!(r.stats, m.stats, "same graph, same stats");
        assert!(layer_ids(&r).contains(&"view".to_string()), "{r:?}");
        let view: Vec<&str> = r
            .cells
            .iter()
            .filter(|c| c.layer.as_str() == "view")
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(view.len(), 1, "A only: {view:?}");

        // A fresh generation: the old matrix's requests fail, the new one's work.
        assert!(r.generation > m.generation);
        let e = s
            .cell_symbols(&paths, None, m.generation, None, &old_cells, None, None)
            .unwrap_err();
        assert_eq!(format!("{e:#}"), "analysis changed, re-run Analyze");
        let view_cells: Vec<String> = view.iter().map(|c| c.to_string()).collect();
        let syms = s
            .cell_symbols(&paths, None, r.generation, None, &view_cells, None, None)
            .unwrap();
        assert_eq!(
            syms.symbols
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        let again = s.reclassify(&paths).unwrap();
        assert!(again.generation > r.generation);
        assert_eq!(again.cells, r.cells);

        // Invalid rules: an error naming the file; the cache is unchanged.
        std::fs::write(
            root.join(pipeline::PROJECT_RULES),
            "[[rule]]\nlayer = \"nope\"\n",
        )
        .unwrap();
        let e = format!("{:#}", s.reclassify(&paths).unwrap_err());
        assert!(e.contains(".structure/rules.toml"), "{e}");
        assert!(
            s.cell_symbols(
                &paths,
                None,
                again.generation,
                None,
                &view_cells,
                None,
                None
            )
            .is_ok()
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn reclassify_needs_an_analysis() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let e = Session::new().reclassify(&[tmp]).unwrap_err();
        assert!(format!("{e:#}").contains("run Analyze"), "{e:#}");
    }

    #[test]
    fn reclassify_keeps_working_tree_fresh_and_drops_snapshots() {
        let (repo, c1, c2) = temp_repo("reclassify-cache");
        let root = repo.join("app");
        let path = root.to_string_lossy().into_owned();
        let paths = vec![path.clone()];
        let s = Session::with_impact_options(no_scip());
        let steps = Mutex::new(Vec::<String>::new());
        let record = |p: Progress| steps.lock().unwrap().push(p.step);
        s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        s.impact(&path, Some(&c1), Some(&c2), &|_| {}).unwrap();
        let before = s.rules_stamp(&paths).unwrap();
        // The cached roots give the stamp a fresh expansion gives.
        assert_eq!(Session::new().rules_stamp(&paths).unwrap(), before);

        std::fs::create_dir_all(root.join(".structure")).unwrap();
        std::fs::write(root.join(pipeline::PROJECT_RULES), VIEW_RULES).unwrap();
        assert_ne!(s.rules_stamp(&paths).unwrap(), before);
        let r = s.reclassify(&paths).unwrap();
        assert!(layer_ids(&r).contains(&"view".to_string()));

        // A rules-only change keeps the working-tree analysis: no re-analysis.
        s.impact(&path, Some(&c1), None, &record).unwrap();
        assert_eq!(
            *steps.lock().unwrap(),
            vec!["computing impact".to_string(), "done".to_string()]
        );
        assert!(
            s.cell_symbols(&paths, None, r.generation, None, &[], None, None)
                .is_ok()
        );
        // The commit snapshot was dropped: analysed again.
        steps.lock().unwrap().clear();
        s.impact(&path, Some(&c1), Some(&c2), &record).unwrap();
        let first = steps.lock().unwrap().first().cloned();
        assert_eq!(first, Some(format!("checking out {}", &c2[..7])));
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// A git repository at a fresh temp dir with `files` committed (c0), then
    /// `class B` added to `java` (c1). Returns the repo and both SHAs.
    fn repo_with(tag: &str, files: &[(&str, &str)], java: &str) -> (PathBuf, String, String) {
        use crate::pipeline::test_support::git_ok;
        let repo =
            std::env::temp_dir().join(format!("structure-session-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        git_ok(&repo, &["init", "-q"]);
        for (rel, text) in files {
            let f = repo.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, text).unwrap();
        }
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "c0"]);
        let c0 = git_ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        let f = repo.join(java);
        let mut text = std::fs::read_to_string(&f).unwrap();
        text.push_str("class B { void b() {} }\n");
        std::fs::write(&f, text).unwrap();
        git_ok(&repo, &["commit", "-q", "-am", "c1"]);
        let c1 = git_ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        (repo, c0, c1)
    }

    fn cell_ids(m: &Matrix) -> Vec<String> {
        let mut ids: Vec<String> = m.cells.iter().map(|c| c.id.clone()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn commit_impact_of_a_multi_language_repo_root_keeps_project_names() {
        let (repo, c0, c1) = repo_with(
            "ml-root",
            &[
                ("pom.xml", "<project/>"),
                ("package.json", "{}"),
                ("src/A.java", "class A { void a() {} }\n"),
                ("web/app.ts", "export class Web { run() {} }\n"),
            ],
            "src/A.java",
        );
        let path = repo.to_string_lossy().into_owned();
        let name = repo.file_name().unwrap().to_string_lossy().into_owned();
        let s = Session::with_impact_options(no_scip());
        let m = s
            .analyze(std::slice::from_ref(&path), &no_scip(), &|_| {})
            .unwrap();
        let names: Vec<&str> = m.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [format!("{name} (java)"), format!("{name} (typescript)")]
        );

        let i = s.impact(&path, Some(&c0), Some(&c1), &|_| {}).unwrap();
        let sm = i.matrix.as_ref().unwrap();
        let snames: Vec<&str> = sm.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(snames, names, "named after the project, not the commit");
        assert_eq!(cell_ids(sm), cell_ids(&m), "same cells as the working tree");
        assert!(!sm.project.contains(&c1[..7]), "{}", sm.project);
        assert_eq!(i.changed_symbols, [format!("{name} (java)::B")], "{i:?}");
        assert!(
            i.changed_cells.iter().all(|c| cell_ids(&m).contains(c)),
            "{i:?}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn impact_of_a_root_expanded_to_one_sub_project() {
        let (repo, c0, c1) = repo_with(
            "one-sub",
            &[
                ("backend/pom.xml", "<project/>"),
                ("backend/src/A.java", "class A { void a() {} }\n"),
            ],
            "backend/src/A.java",
        );
        let path = repo.to_string_lossy().into_owned();
        let s = Session::with_impact_options(no_scip());
        // Working tree (c1) against c0: B was added.
        let i = s.impact(&path, Some(&c0), None, &|_| {}).unwrap();
        assert_eq!(i.changed_files, ["backend/src/A.java"]);
        assert_eq!(i.changed_symbols, ["B"], "{i:?}");
        assert!(!i.changed_cells.is_empty(), "{i:?}");
        // The commit path expands the root as well.
        let i = s.impact(&path, Some(&c0), Some(&c1), &|_| {}).unwrap();
        assert_eq!(i.changed_files, ["backend/src/A.java"]);
        assert_eq!(i.changed_symbols, ["B"], "{i:?}");
        assert!(!i.changed_cells.is_empty(), "{i:?}");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn working_tree_impact_with_invalid_rules_keeps_the_analysis() {
        let (repo, c1, _) = temp_repo("impact-invalid-rules");
        let root = repo.join("app");
        let path = root.to_string_lossy().into_owned();
        let paths = vec![path.clone()];
        let s = Session::with_impact_options(no_scip());
        let m = s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        std::fs::create_dir_all(root.join(".structure")).unwrap();
        std::fs::write(
            root.join(pipeline::PROJECT_RULES),
            "[[rule]]\nlayer = \"nope\"\n",
        )
        .unwrap();
        let e = format!(
            "{:#}",
            s.impact(&path, Some(&c1), None, &|_| {}).unwrap_err()
        );
        assert!(e.contains(".structure/rules.toml"), "{e}");
        // The matrix on screen still has its analysis.
        assert!(
            s.cell_symbols(&paths, None, m.generation, None, &[], None, None)
                .is_ok()
        );
        assert!(
            s.matrix(&paths, None, m.generation, "layer-package")
                .is_ok()
        );
        // Once the rules are fixed, reclassify works on it.
        std::fs::write(root.join(pipeline::PROJECT_RULES), VIEW_RULES).unwrap();
        assert!(layer_ids(&s.reclassify(&paths).unwrap()).contains(&"view".to_string()));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn rules_edited_while_reclassifying_leave_the_entry_stale() {
        let (repo, c1, _) = temp_repo("reclassify-race");
        let root = repo.join("app");
        let path = root.to_string_lossy().into_owned();
        let paths = vec![path.clone()];
        let s = Session::with_impact_options(no_scip());
        s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        std::fs::create_dir_all(root.join(".structure")).unwrap();
        let rules = root.join(pipeline::PROJECT_RULES);
        std::fs::write(&rules, VIEW_RULES).unwrap();
        // Edited after being read, before the reclassified entry is stored.
        let edit = || std::fs::write(&rules, format!("{VIEW_RULES}# edited\n")).unwrap();
        s.reclassify_then(&paths, &edit).unwrap();
        let (_, key) = canonical(&path).unwrap();
        let (_, stored, _, _) = s.cache.get(&key).unwrap().unwrap();
        assert_ne!(
            stored,
            combined_fingerprint(&[root.canonicalize().unwrap()])
        );
        // So the next working-tree impact analyses it again.
        let steps = Mutex::new(Vec::<String>::new());
        s.impact(&path, Some(&c1), None, &|p| {
            steps.lock().unwrap().push(p.step)
        })
        .unwrap();
        assert_ne!(
            *steps.lock().unwrap(),
            vec!["computing impact".to_string(), "done".to_string()]
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn reclassify_drops_the_snapshots_of_each_root() {
        use crate::pipeline::test_support::git_ok;
        let (repo, _, c2) = temp_repo("reclassify-multi");
        std::fs::create_dir_all(repo.join("lib/src")).unwrap();
        std::fs::write(repo.join("lib/pom.xml"), "<project/>").unwrap();
        std::fs::write(repo.join("lib/src/L.java"), "class L {}\n").unwrap();
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "lib"]);
        let c3 = git_ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        let app = repo.join("app").to_string_lossy().into_owned();
        let lib = repo.join("lib").to_string_lossy().into_owned();
        let paths = vec![app.clone(), lib.clone()];
        let s = Session::with_impact_options(no_scip());
        s.analyze(&paths, &no_scip(), &|_| {}).unwrap();
        s.impact(&app, Some(&c2), Some(&c3), &|_| {}).unwrap();
        s.impact(&lib, Some(&c2), Some(&c3), &|_| {}).unwrap();
        let snaps = |s: &Session| {
            s.cache
                .with(|v| v.iter().filter(|e| e.snapshot).count())
                .unwrap()
        };
        assert_eq!(snaps(&s), 2);
        s.reclassify(&paths).unwrap();
        assert_eq!(snaps(&s), 0, "every root's snapshots dropped");
        let _ = std::fs::remove_dir_all(&repo);
    }
}
