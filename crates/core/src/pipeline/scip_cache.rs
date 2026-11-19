//! Content-addressed cache of SCIP documents, shared by every worktree of a
//! repository: `<git-common-dir>/structure/scip-docs/<indexer>/<blob[..2]>/<blob>-<path hash>`.
//!
//! A SCIP document depends on its file's content (and, rarely, on how its
//! dependencies resolve) and names symbols globally, not by position. So the
//! document indexed for `path` at blob `b` is reused in any commit where
//! `path` still has blob `b`, instead of running the indexer again. The path
//! is part of the key: TS symbols embed it, so the same content elsewhere is
//! another document. The indexer (and its version) is too.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::Context;

use crate::impact;
use model_scip as scip;
use model_scip::ScipIndex;

/// How much of a commit's source files the composed index covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub covered: usize,
    pub total: usize,
    /// Source files without a cached document, sorted.
    pub missing: Vec<String>,
}

impl Coverage {
    pub fn warning(&self) -> String {
        const SHOWN: usize = 5;
        let mut w = format!(
            "SCIP data reused from earlier analyses for {} of {} source files (indexer not run)",
            self.covered, self.total
        );
        if !self.missing.is_empty() {
            let more = self.missing.len().saturating_sub(SHOWN);
            w.push_str(&format!(
                "; calls in the others come from tree-sitter only: {}{}",
                self.missing[..self.missing.len().min(SHOWN)].join(", "),
                if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                }
            ));
        }
        w
    }
}

/// FNV-1a: a stable (across builds and platforms) hash for file names.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Cache directory for documents of indexer `tag` in the repository
/// containing `root`; `None` outside git.
fn cache_dir(root: &Path, tag: &str) -> Option<PathBuf> {
    Some(super::structure_dir(root).ok()?.join("scip-docs").join(tag))
}

fn entry(dir: &Path, path: &str, blob: &str) -> PathBuf {
    dir.join(&blob[..2.min(blob.len())])
        .join(format!("{blob}-{:016x}", fnv(path)))
}

fn has_ext(path: &str, exts: &[&str]) -> bool {
    Path::new(path)
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| exts.contains(&x))
}

/// Stamp identifying one state of `index`: a file named after its path,
/// holding its size and mtime.
fn stamp(index: &Path) -> anyhow::Result<(String, String)> {
    let meta = std::fs::metadata(index)?;
    let mtime = meta
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let canon = index.canonicalize()?;
    Ok((
        format!("{:016x}", fnv(&canon.to_string_lossy())),
        format!("{} {mtime}", meta.len()),
    ))
}

/// Writes `bytes` to `target` through a unique temporary file, so readers
/// never see a partial file and concurrent writers never share one.
fn write_atomic(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let parent = target.parent().context("cache entry has no parent")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let tmp = target.with_extension(format!(
        "tmp{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let res = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, target));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res.with_context(|| format!("writing {}", target.display()))
}

/// Stores the documents of `index` (at the root of `root`, produced by
/// indexer `tag`) for the source files not modified since it was written.
/// Returns the number of new entries; does nothing outside git or when this
/// index was already stored.
pub fn store(root: &Path, index: &Path, exts: &[&str], tag: &str) -> anyhow::Result<usize> {
    let Some(dir) = cache_dir(root, tag) else {
        return Ok(0);
    };
    let (stamp_name, stamp_val) = stamp(index)?;
    let stamp_file = dir.join("stamps").join(stamp_name);
    if std::fs::read_to_string(&stamp_file).is_ok_and(|s| s == stamp_val) {
        return Ok(0);
    }
    // A file edited while the indexer ran has an mtime before the index's
    // and may be stored with an older document: accepted, as for a stale
    // index.scip.
    let indexed = std::fs::metadata(index)?.modified()?;

    // Tracked and untracked (non-ignored) sources not newer than the index.
    let listed = impact::git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut paths: Vec<String> = listed
        .split('\0')
        .filter(|p| !p.is_empty() && !p.contains('\n') && has_ext(p, exts))
        .filter(|p| {
            std::fs::metadata(root.join(p))
                .and_then(|m| m.modified())
                .is_ok_and(|t| t <= indexed)
        })
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    let blobs: HashMap<String, String> = paths
        .iter()
        .cloned()
        .zip(impact::hash_paths(root, &paths)?)
        .collect();

    let mut added = 0usize;
    scip::for_each_raw_document(index, &mut |raw| {
        let path = scip::raw_document_path(&raw)?;
        let Some(blob) = blobs.get(&path) else {
            return Ok(());
        };
        let target = entry(&dir, &path, blob);
        if !target.is_file() {
            write_atomic(&target, &raw)?;
            added += 1;
        }
        Ok(())
    })?;
    write_atomic(&stamp_file, stamp_val.as_bytes())?;
    Ok(added)
}

/// SCIP index of the clean checkout at `root` (a snapshot) built from the
/// cached documents of indexer `tag`, with its coverage; `None` when no
/// document is cached.
pub fn compose(
    root: &Path,
    exts: &[&str],
    tag: &str,
) -> anyhow::Result<Option<(ScipIndex, Coverage)>> {
    let Some(dir) = cache_dir(root, tag) else {
        return Ok(None);
    };
    // Paths are relative to `root` (ls-tree's default), as in index.scip.
    let tree = impact::git(root, &["ls-tree", "-r", "-z", "HEAD", "--", "."])?;
    let mut candidates: Vec<(&str, PathBuf)> = Vec::new();
    for rec in tree.split('\0').filter(|r| !r.is_empty()) {
        let Some((meta, path)) = rec.split_once('\t') else {
            continue;
        };
        let mut f = meta.split(' ');
        let (Some(_mode), Some("blob"), Some(blob)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        if has_ext(path, exts) {
            candidates.push((path, entry(&dir, path, blob)));
        }
    }
    // Each entry is read once and decoded in bounded batches. An unreadable
    // or damaged entry (its path does not decode or is another file's)
    // counts as missing rather than failing the analysis.
    let mut missing: Vec<String> = Vec::new();
    let mut covered = 0usize;
    let index = scip::from_raw_documents(candidates.iter().filter_map(|(path, e)| {
        let raw = std::fs::read(e)
            .ok()
            .filter(|raw| scip::raw_document_path(raw).is_ok_and(|p| p == *path));
        if raw.is_some() {
            covered += 1;
        } else {
            missing.push(path.to_string());
        }
        raw.map(Ok)
    }))?;
    if covered == 0 {
        return Ok(None);
    }
    missing.sort();
    let coverage = Coverage {
        covered,
        total: covered + missing.len(),
        missing,
    };
    Ok(Some((index, coverage)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EdgeKind, Language};
    use crate::pipeline::test_support::*;
    use crate::pipeline::{Options, analyze, analyze_snapshot, scip_cache_tag, snapshot};
    use ::scip::types::{Document, Index, Occurrence};
    use protobuf::Message;
    use std::time::Duration;

    const A: &str = "semanticdb maven . . A#a().";
    const B: &str = "semanticdb maven . . B#b().";
    const EXTS: &[&str] = &["java"];

    fn tag() -> String {
        scip_cache_tag(Language::Java)
    }

    /// index.scip for c1's `src/A.java`: defines `A#a()` and `B#b()`; with
    /// `call`, `b` calls `a` (a fake reference inside `b`'s body).
    fn write_index(dir: &Path, call: bool) {
        let occ = |sym: &str, range: Vec<i32>, roles: i32| Occurrence {
            symbol: sym.into(),
            range,
            symbol_roles: roles,
            ..Default::default()
        };
        let mut d = Document::new();
        d.relative_path = "src/A.java".into();
        d.occurrences.push(occ(A, vec![0, 15, 16], 1));
        d.occurrences.push(occ(B, vec![1, 15, 16], 1));
        if call {
            d.occurrences.push(occ(A, vec![1, 19, 20], 0));
        }
        let mut idx = Index::new();
        idx.documents.push(d);
        std::fs::write(dir.join("index.scip"), idx.write_to_bytes().unwrap()).unwrap();
    }

    fn calls(a: &crate::pipeline::Analysis) -> usize {
        a.graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Call)
            .count()
    }

    fn set_mtime(p: &Path, t: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn offline() -> Options {
        Options {
            reindex_if_stale: false,
            ..Options::default()
        }
    }

    #[test]
    fn documents_cached_then_reused_for_a_commit() {
        let (repo, c1, c2) = temp_repo("scipcache");
        let root = repo.join("app");
        // A first, exact analysis of c1 fills the cache from its index.
        let snap = snapshot(&root, &c1).unwrap();
        write_index(&snap, true);
        let a = analyze(&snap, &offline(), &|_| {}).unwrap();
        assert_eq!(calls(&a), 1, "{:?}", a.graph.edges);
        assert!(
            repo.join(".git/structure/scip-docs").join(tag()).is_dir(),
            "{:?}",
            a.warnings
        );
        // Stored once per index state.
        assert_eq!(
            store(&snap, &snap.join("index.scip"), EXTS, &tag()).unwrap(),
            0
        );

        // Without its index, c1's snapshot is rebuilt from the cache: same
        // call edge, no indexer, coverage reported.
        std::fs::remove_file(snap.join("index.scip")).unwrap();
        let (_, b) = analyze_snapshot(&root, &c1, &offline(), &|_| {}).unwrap();
        assert_eq!(calls(&b), 1, "{:?}", b.warnings);
        assert!(
            b.warnings
                .iter()
                .any(|w| w.contains("for 1 of 1 source files")),
            "{:?}",
            b.warnings
        );
        assert!(
            !b.warnings
                .iter()
                .any(|w| w.contains("indexing") || w.contains("no SCIP indexer")),
            "{:?}",
            b.warnings
        );
        assert!(
            !snap.join("index.scip").exists(),
            "nothing written to the snapshot"
        );

        // A snapshot with its own index uses it, not the cache.
        write_index(&snap, false);
        let (_, c) = analyze_snapshot(&root, &c1, &offline(), &|_| {}).unwrap();
        assert_eq!(calls(&c), 0);
        assert!(
            !c.warnings.iter().any(|w| w.contains("source files")),
            "{:?}",
            c.warnings
        );

        // c2 changed A.java: its blob was never indexed.
        let snap2 = snapshot(&root, &c2).unwrap();
        assert!(compose(&snap2, EXTS, &tag()).unwrap().is_none());
        // Another indexer (version) has its own documents.
        assert!(compose(&snap, EXTS, "scip-java-0.0.0").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn files_modified_after_the_index_are_not_stored_and_a_new_index_is() {
        let (repo, c1, _) = temp_repo("scipstale");
        let root = repo.join("app");
        let snap = snapshot(&root, &c1).unwrap();
        write_index(&snap, true);
        let index = snap.join("index.scip");
        let t0 = std::fs::metadata(&index).unwrap().modified().unwrap();
        // Same content, but touched after the index: not trusted.
        set_mtime(&snap.join("src/A.java"), t0 + Duration::from_secs(60));
        assert_eq!(store(&snap, &index, EXTS, &tag()).unwrap(), 0);
        assert!(compose(&snap, EXTS, &tag()).unwrap().is_none());
        // A newer state of the index is stored again.
        set_mtime(&index, t0 + Duration::from_secs(120));
        assert_eq!(store(&snap, &index, EXTS, &tag()).unwrap(), 1);
        assert!(compose(&snap, EXTS, &tag()).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn coverage_lists_missing_files_and_same_blob_elsewhere_is_not_reused() {
        let (repo, c1, _) = temp_repo("scipcover");
        let root = repo.join("app");
        let snap = snapshot(&root, &c1).unwrap();
        write_index(&snap, true);
        store(&snap, &snap.join("index.scip"), EXTS, &tag()).unwrap();
        // c3: a copy of A.java (same blob, other path) and a new file.
        git_ok(&repo, &["checkout", "-q", &c1, "--", "app/src/A.java"]);
        std::fs::copy(root.join("src/A.java"), root.join("src/Copy.java")).unwrap();
        std::fs::write(root.join("src/N.java"), "class N {}\n").unwrap();
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "c3"]);
        let c3 = git_ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        let snap3 = snapshot(&root, &c3).unwrap();
        let (idx, cov) = compose(&snap3, EXTS, &tag()).unwrap().unwrap();
        assert_eq!(idx.documents.len(), 1);
        assert_eq!(
            cov,
            Coverage {
                covered: 1,
                total: 3,
                missing: vec!["src/Copy.java".into(), "src/N.java".into()],
            }
        );
        // Uncovered files are still parsed; the warning reaches the analysis.
        let (_, a) = analyze_snapshot(&root, &c3, &offline(), &|_| {}).unwrap();
        assert!(a.graph.symbols.iter().any(|s| s.name == "N"));
        assert!(
            a.warnings
                .iter()
                .any(|w| w.contains("1 of 3") && w.contains("src/N.java")),
            "{:?}",
            a.warnings
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn damaged_entries_are_missing_or_an_error_never_a_wrong_document() {
        let (repo, c1, _) = temp_repo("scipdamaged");
        let root = repo.join("app");
        let snap = snapshot(&root, &c1).unwrap();
        write_index(&snap, true);
        store(&snap, &snap.join("index.scip"), EXTS, &tag()).unwrap();
        let blob = git_ok(&snap, &["rev-parse", "HEAD:./src/A.java"])
            .trim()
            .to_string();
        let dir = cache_dir(&snap, &tag()).unwrap();
        let e = entry(&dir, "src/A.java", &blob);
        assert!(e.is_file());
        // Broken framing: counted as missing.
        std::fs::write(&e, [0xff, 0xff]).unwrap();
        assert!(compose(&snap, EXTS, &tag()).unwrap().is_none());
        // Right path, broken body: an error (the analysis then indexes as usual).
        let mut raw = vec![0x0a, 10];
        raw.extend_from_slice(b"src/A.java");
        raw.extend_from_slice(&[0x12, 0x01, 0xff]);
        std::fs::write(&e, raw).unwrap();
        assert!(compose(&snap, EXTS, &tag()).is_err());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn eol_converted_checkouts_hit_the_committed_blob() {
        let (repo, _, _) = temp_repo("scipcrlf");
        let root = repo.join("app");
        std::fs::write(repo.join(".gitattributes"), "*.java text eol=crlf\n").unwrap();
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "crlf"]);
        let c = git_ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        let snap = snapshot(&root, &c).unwrap();
        assert!(
            std::fs::read(snap.join("src/A.java"))
                .unwrap()
                .contains(&b'\r')
        );
        write_index(&snap, false);
        assert_eq!(
            store(&snap, &snap.join("index.scip"), EXTS, &tag()).unwrap(),
            1
        );
        std::fs::remove_file(snap.join("index.scip")).unwrap();
        assert!(compose(&snap, EXTS, &tag()).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn outside_git_nothing_is_cached() {
        let dir = std::env::temp_dir().join(format!("structure-nogit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        write_index(&dir, true);
        assert_eq!(
            store(&dir, &dir.join("index.scip"), EXTS, &tag()).unwrap(),
            0
        );
        assert!(compose(&dir, EXTS, &tag()).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn coverage_warning_lists_five_missing_files() {
        let cov = Coverage {
            covered: 3,
            total: 10,
            missing: (1..=7).map(|i| format!("f{i}.java")).collect(),
        };
        assert_eq!(
            cov.warning(),
            "SCIP data reused from earlier analyses for 3 of 10 source files (indexer not run); \
             calls in the others come from tree-sitter only: \
             f1.java, f2.java, f3.java, f4.java, f5.java (+2 more)"
        );
    }
}
