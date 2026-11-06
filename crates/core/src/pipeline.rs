//! End-to-end pipeline: detect language, index (SCIP), parse, link,
//! classify, build the matrix, compute impact.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::classify::{RuleSet, classify};
use crate::impact;
use crate::ir::{Graph, Language};
use crate::link::{LinkStats, link};
use crate::matrix;
use crate::model::{Classification, Commit, History, Impact, Matrix, Progress};
use crate::{java, scip, ts};

pub const SCIP_JAVA_VERSION: &str = "0.12.3";
pub const SCIP_TS_VERSION: &str = "0.6.1";

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Always re-run the indexer.
    pub reindex: bool,
    /// Re-run the indexer when a source file is newer than `index.scip`.
    /// When false, a stale index is reused with a warning.
    pub reindex_if_stale: bool,
    pub use_scip: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            reindex: false,
            reindex_if_stale: true,
            use_scip: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub graph: Graph,
    pub classification: Classification,
    pub matrix: Matrix,
    pub link_stats: Option<LinkStats>,
    pub warnings: Vec<String>,
}

pub fn detect_language(root: &Path) -> anyhow::Result<Language> {
    let has = |n: &str| root.join(n).is_file();
    if has("pom.xml")
        || has("build.gradle")
        || has("build.gradle.kts")
        || has("settings.gradle")
        || has("settings.gradle.kts")
    {
        Ok(Language::Java)
    } else if has("tsconfig.json") || has("package.json") {
        Ok(Language::TypeScript)
    } else {
        bail!(
            "cannot detect language in {}: expected pom.xml/build.gradle/build.gradle.kts/settings.gradle/settings.gradle.kts (Java) or tsconfig.json/package.json (TypeScript)",
            root.display()
        )
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

fn find_cs() -> Option<PathBuf> {
    find_on_path("cs").or_else(|| {
        let p = PathBuf::from(std::env::var_os("HOME")?).join(".local/bin/cs");
        p.is_file().then_some(p)
    })
}

fn run_indexer(root: &Path, mut cmd: Command) -> Result<(), String> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::Duration;
    let mut child = cmd
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {:?}: {e}", cmd.get_program()))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(INDEX_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "timed out after {} min; check ~/.m2/settings.xml (unreachable repository?) or run the indexer manually",
                    INDEX_TIMEOUT_SECS / 60
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("waiting for indexer: {e}")),
        }
    };
    if status.success() {
        return Ok(());
    }
    let buf = reader.join().unwrap_or_default();
    let err = String::from_utf8_lossy(&buf);
    let mut tail: Vec<&str> = err.lines().rev().take(8).collect();
    tail.reverse();
    Err(format!(
        "{} ({status}); for Java also check ~/.m2/settings.xml",
        tail.join(" | ")
    ))
}

const INDEX_TIMEOUT_SECS: u64 = 600;

/// Number of source files modified after `index` was written.
fn newer_sources(root: &Path, lang: Language, index: &Path) -> usize {
    let Ok(indexed) = std::fs::metadata(index).and_then(|m| m.modified()) else {
        return 0;
    };
    let exts: &[&str] = match lang {
        Language::Java => &["java"],
        Language::TypeScript => &["ts", "tsx"],
    };
    ignore::WalkBuilder::new(root)
        // `.gradle` is explicit: keeps it out even if `.hidden(false)` is set later.
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some("node_modules" | "target" | "build" | "dist" | ".git" | ".gradle")
            )
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| exts.contains(&x))
        })
        .filter(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t > indexed)
        })
        .count()
}

/// Returns the path of an up-to-date `index.scip`, or `Ok(None)` (with a
/// warning through `warn`) if it could not be produced.
pub fn ensure_index(
    root: &Path,
    lang: Language,
    opts: &Options,
    warn: &mut dyn FnMut(String),
) -> anyhow::Result<Option<PathBuf>> {
    let index = root.join("index.scip");
    if index.is_file() && !opts.reindex {
        let stale = newer_sources(root, lang, &index);
        if stale == 0 {
            return Ok(Some(index));
        }
        if !opts.reindex_if_stale {
            warn(format!(
                "index.scip is older than {stale} source file(s); call edges in those files may be off until the project is re-analyzed"
            ));
            return Ok(Some(index));
        }
    }
    let cmd = match lang {
        Language::Java => {
            if let Some(p) = find_on_path("scip-java") {
                let mut c = Command::new(p);
                c.args(["index", "--output", "index.scip"]);
                Some(c)
            } else if let Some(cs) = find_cs() {
                let mut c = Command::new(cs);
                c.arg("launch")
                    .arg(format!(
                        "com.sourcegraph:scip-java_2.13:{SCIP_JAVA_VERSION}"
                    ))
                    .args(["--", "index", "--output", "index.scip"]);
                Some(c)
            } else {
                None
            }
        }
        Language::TypeScript => find_on_path("npx").map(|p| {
            let mut c = Command::new(p);
            c.args(["-y"])
                .arg(format!("@sourcegraph/scip-typescript@{SCIP_TS_VERSION}"))
                .args(["index", "--output", "index.scip"]);
            c
        }),
    };
    let Some(cmd) = cmd else {
        warn("no SCIP indexer found (scip-java/cs/npx); using tree-sitter only".into());
        return Ok(None);
    };
    match run_indexer(root, cmd) {
        Ok(()) if index.is_file() => Ok(Some(index)),
        Ok(()) => {
            warn("indexer succeeded but produced no index.scip; using tree-sitter only".into());
            Ok(None)
        }
        Err(e) => {
            warn(format!("indexing failed, using tree-sitter only: {e}"));
            Ok(None)
        }
    }
}

fn project_name(root: &Path) -> String {
    let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
}

pub fn analyze(
    root: &Path,
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Analysis> {
    let started = Instant::now();
    let step = |s: &str, pct: u8| {
        progress(Progress {
            step: s.into(),
            pct,
        })
    };
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let lang = detect_language(root)?;
    let mut warnings: Vec<String> = Vec::new();

    step("indexing", 5);
    let index_path = if opts.use_scip {
        ensure_index(root, lang, opts, &mut |w| warnings.push(w))?
    } else {
        None
    };

    step("parsing", 35);
    let graph = match lang {
        Language::Java => java::parse_project(root),
        Language::TypeScript => ts::parse_project(root),
    }
    .with_context(|| format!("parsing {}", root.display()))?;

    step("linking", 60);
    let (graph, link_stats) = match index_path {
        Some(p) => match scip::load(&p) {
            Ok(idx) => {
                let (g, st) = link(graph, &idx);
                (g, Some(st))
            }
            Err(e) => {
                warnings.push(format!(
                    "cannot read {}: {e:#}; using tree-sitter only",
                    p.display()
                ));
                (graph, None)
            }
        },
        None => (graph, None),
    };

    step("classifying", 80);
    let mut rules = RuleSet::builtin(lang)?;
    let custom = root.join(".structure/rules.toml");
    if custom.is_file() {
        let text = std::fs::read_to_string(&custom)
            .with_context(|| format!("reading {}", custom.display()))?;
        let user = RuleSet::from_toml_str(&text)
            .with_context(|| format!("parsing {}", custom.display()))?;
        rules = rules.merge(user);
    }
    let classification = classify(&graph, &rules);

    step("building matrix", 95);
    let elapsed = started.elapsed().as_millis() as u64;
    let mut matrix = matrix::build(&graph, &classification, &project_name(root), elapsed);
    matrix.warnings = warnings.clone();
    step("done", 100);

    Ok(Analysis {
        graph,
        classification,
        matrix,
        link_stats,
        warnings,
    })
}

/// Recent commits reachable from HEAD (`--topo-order`, newest first) plus the dirty flag.
pub fn history(root: &Path, limit: u32) -> anyhow::Result<History> {
    const FMT: &str = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%aI%x1f%D%x1f%s%x1e";
    let n = format!("-n{}", limit.max(1));
    let log = impact::git(
        root,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "--no-show-signature",
            "--no-color",
            "--topo-order",
            &n,
            FMT,
            "HEAD",
            "--",
        ],
    )
    .context("cannot read history (not a git repository, or no commits yet)")?;
    let mut empty_tree: Option<String> = None;
    let mut commits = Vec::new();
    for rec in log.split('\x1e').map(|r| r.trim_start_matches('\n')) {
        if rec.is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.splitn(7, '\x1f').collect();
        let [id, short, parents, author, date, refs, subject] = f[..] else {
            bail!("unexpected log record: {rec:?}");
        };
        let parents: Vec<String> = parents.split_whitespace().map(str::to_string).collect();
        let base = match parents.first() {
            Some(p) => p.clone(),
            None => match &empty_tree {
                Some(t) => t.clone(),
                None => {
                    let t = impact::git(root, &["hash-object", "-t", "tree", "/dev/null"])?
                        .trim()
                        .to_string();
                    empty_tree = Some(t.clone());
                    t
                }
            },
        };
        commits.push(Commit {
            id: id.to_string(),
            short: short.to_string(),
            parents,
            base,
            subject: subject.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            refs: refs
                .split(", ")
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    if commits.is_empty() {
        bail!("no commits in this repository");
    }
    // Same changes as `impact(None, None)`: tracked edits and non-ignored
    // untracked files under the project root, minus the index we write.
    let dirty = impact::git(root, &["status", "--porcelain", "-uall", "--", "."])?
        .lines()
        .any(|l| {
            let p = l.get(3..).unwrap_or("").trim_matches('"');
            p != "index.scip" && !p.ends_with("/index.scip")
        });
    Ok(History { dirty, commits })
}

pub fn impact(
    root: &Path,
    analysis: &Analysis,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Impact> {
    let mut changes = impact::changed_lines(root, base, head)?;
    // The SCIP index we write into the project is a build artefact, not a change.
    changes.retain(|c| c.file != "index.scip");
    let mut result = impact::compute(
        &analysis.graph,
        &analysis.classification,
        &changes,
        base.unwrap_or("HEAD"),
        head,
    );
    result.warnings = analysis.warnings.clone();
    if let Some(h) = head {
        result.warnings.push(format!(
            "symbol positions come from the working tree; results for {h} are exact only if it is checked out"
        ));
    }
    Ok(result)
}

/// Most snapshots kept per repository; the least recently used are removed.
pub const MAX_SNAPSHOTS: usize = 8;

/// Project root of a detached worktree of `rev`, created on first use and
/// reused afterwards. The user's working tree, index, HEAD and stash are untouched.
pub fn snapshot(root: &Path, rev: &str) -> anyhow::Result<PathBuf> {
    snapshot_bounded(root, rev, MAX_SNAPSHOTS)
}

fn snapshot_bounded(root: &Path, rev: &str, max: usize) -> anyhow::Result<PathBuf> {
    let spec = format!("{rev}^{{commit}}");
    let sha = impact::git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .with_context(|| format!("unknown revision: {rev}"))?
    .trim()
    .to_string();
    let prefix = impact::git(root, &["rev-parse", "--show-prefix"])?
        .trim()
        .to_string();
    let common = impact::git(root, &["rev-parse", "--git-common-dir"])?;
    let snaps = root.join(common.trim()).join("structure").join("snapshots");
    std::fs::create_dir_all(&snaps).with_context(|| format!("creating {}", snaps.display()))?;
    let snaps = snaps.canonicalize().unwrap_or(snaps);
    let dir = snaps.join(&sha);
    let dir_str = dir.to_string_lossy().into_owned();

    let valid = dir.join(".git").exists()
        && impact::git(&dir, &["rev-parse", "--show-toplevel"])
            .is_ok_and(|t| Path::new(t.trim()).canonicalize().ok() == dir.canonicalize().ok())
        && impact::git(&dir, &["rev-parse", "HEAD"]).is_ok_and(|h| h.trim() == sha)
        // Tracked files deleted from the checkout; untracked ones (index.scip, builds) are fine.
        && impact::git(&dir, &["ls-files", "-d"]).is_ok_and(|d| d.trim().is_empty());
    if !valid {
        let _ = impact::git(root, &["worktree", "remove", "--force", &dir_str]);
        let _ = std::fs::remove_dir_all(&dir);
        impact::git(root, &["worktree", "prune"])?;
        impact::git(root, &["worktree", "add", "--detach", &dir_str, &sha])
            .with_context(|| format!("creating snapshot of {rev}"))?;
    }
    // Mark as recently used, with a marker outside the worktree (the indexer
    // writes inside it, which would change the dir's mtime).
    let _ = std::fs::write(marker(&snaps, &sha), "");
    prune_snapshots(root, &snaps, max, &sha);
    Ok(dir.join(prefix))
}

fn marker(snaps: &Path, sha: &str) -> PathBuf {
    snaps.join(format!("{sha}.used"))
}

/// Remove least recently used snapshots beyond `max`, never `keep`.
fn prune_snapshots(root: &Path, snaps: &Path, max: usize, keep: &str) {
    let Ok(rd) = std::fs::read_dir(snaps) else {
        return;
    };
    let mut all: Vec<(std::time::SystemTime, PathBuf)> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let used = e.file_name().to_str().map(|n| marker(snaps, n));
            let t = used
                .and_then(|m| std::fs::metadata(m).ok())
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            (t, e.path())
        })
        .collect();
    if all.len() <= max {
        return;
    }
    all.sort();
    let excess = all.len() - max;
    for (_, p) in all
        .into_iter()
        .filter(|(_, p)| p.file_name().is_none_or(|n| n != keep))
        .take(excess)
    {
        let _ = impact::git(
            root,
            &["worktree", "remove", "--force", &p.to_string_lossy()],
        );
        let _ = std::fs::remove_dir_all(&p);
        if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
            let _ = std::fs::remove_file(marker(snaps, n));
        }
    }
    let _ = impact::git(root, &["worktree", "prune"]);
}

/// Impact of `base..head`. With a `head`, the commit is analysed exactly in
/// its snapshot; without, the working tree is used as in [`impact`].
pub fn impact_at(
    root: &Path,
    base: Option<&str>,
    head: Option<&str>,
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Impact> {
    if head.is_some() && base.is_none() {
        bail!("head requires base");
    }
    let Some(h) = head else {
        let analysis = analyze(root, opts, progress)?;
        return impact(root, &analysis, base, None);
    };
    let base_sha = match base {
        Some(b) => Some(
            impact::git(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{b}^{{commit}}"),
                ],
            )
            .with_context(|| format!("unknown revision: {b}"))?
            .trim()
            .to_string(),
        ),
        None => None,
    };
    let base = base_sha.as_deref();
    let snap = snapshot(root, h)?;
    let analysis = analyze(&snap, opts, progress)?;
    let mut changes = impact::changed_lines(&snap, base, head)?;
    changes.retain(|c| c.file != "index.scip");
    let mut result = impact::compute(
        &analysis.graph,
        &analysis.classification,
        &changes,
        base.unwrap_or("HEAD"),
        head,
    );
    result.warnings = analysis.warnings;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn touch(p: &Path, t: SystemTime) {
        std::fs::write(p, "x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn git_ok(dir: &Path, args: &[&str]) -> String {
        let cfg = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ];
        let all: Vec<&str> = cfg.iter().chain(args.iter()).copied().collect();
        impact::git(dir, &all).unwrap()
    }

    /// Repo with a Java project in `app/`. c0: class A (line 1); c1 adds class B
    /// (line 2); c2 prepends a comment line, so B moves to line 3.
    fn temp_repo(tag: &str) -> (PathBuf, String, String) {
        let dir = std::env::temp_dir().join(format!("structure-snap-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app/src")).unwrap();
        git_ok(&dir, &["init", "-q"]);
        std::fs::write(dir.join("app/pom.xml"), "<project/>").unwrap();
        let f = dir.join("app/src/A.java");
        std::fs::write(&f, "class A { void a() {} }\n").unwrap();
        git_ok(&dir, &["add", "."]);
        git_ok(&dir, &["commit", "-q", "-m", "c0"]);
        std::fs::write(&f, "class A { void a() {} }\nclass B { void b() {} }\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "c1"]);
        let c1 = git_ok(&dir, &["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(
            &f,
            "// h\nclass A { void a() {} }\nclass B { void b() {} }\n",
        )
        .unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "c2"]);
        let c2 = git_ok(&dir, &["rev-parse", "HEAD"]).trim().to_string();
        (dir, c1, c2)
    }

    fn no_scip() -> Options {
        Options {
            use_scip: false,
            ..Options::default()
        }
    }

    #[test]
    fn snapshot_created_reused_and_invalid_rev() {
        let (repo, c1, _) = temp_repo("reuse");
        let root = repo.join("app");
        let before = git_ok(&repo, &["status", "--porcelain"]);
        let head = git_ok(&repo, &["rev-parse", "HEAD"]);
        let a = snapshot(&root, &c1).unwrap();
        assert!(a.ends_with("app") && a.join("pom.xml").is_file());
        assert!(a.join("src/A.java").is_file());
        std::fs::write(a.join("marker"), "").unwrap();
        let b = snapshot(&root, &c1[..8]).unwrap();
        assert_eq!(a, b);
        assert!(b.join("marker").is_file(), "snapshot reused, not recreated");
        // Broken snapshot is recreated.
        std::fs::remove_dir_all(a.parent().unwrap()).unwrap();
        let c = snapshot(&root, &c1).unwrap();
        assert!(c.join("src/A.java").is_file());
        assert!(snapshot(&root, "no-such-rev").is_err());
        assert_eq!(git_ok(&repo, &["status", "--porcelain"]), before);
        assert_eq!(git_ok(&repo, &["rev-parse", "HEAD"]), head);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn broken_snapshot_at_users_head_is_recreated() {
        let (repo, _, c2) = temp_repo("broken");
        let root = repo.join("app");
        let a = snapshot(&root, &c2).unwrap();
        let top = a.parent().unwrap().to_path_buf();
        std::fs::remove_file(top.join(".git")).unwrap();
        let b = snapshot(&root, &c2).unwrap();
        assert!(top.join(".git").exists() && b.join("src/A.java").is_file());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn damaged_snapshot_is_recreated() {
        let (repo, c1, _) = temp_repo("damaged");
        let root = repo.join("app");
        let a = snapshot(&root, &c1).unwrap();
        std::fs::write(a.join("index.scip"), "x").unwrap();
        snapshot(&root, &c1).unwrap();
        assert!(
            a.join("index.scip").is_file(),
            "untracked files do not invalidate"
        );
        std::fs::remove_file(a.join("src/A.java")).unwrap();
        let b = snapshot(&root, &c1).unwrap();
        assert!(b.join("src/A.java").is_file());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn reuse_refreshes_recency_for_pruning() {
        let (repo, c1, c2) = temp_repo("lru");
        let root = repo.join("app");
        let c0 = format!("{c1}^");
        let nap = || std::thread::sleep(Duration::from_millis(30));
        let s1 = snapshot_bounded(&root, &c1, 2).unwrap();
        nap();
        let s2 = snapshot_bounded(&root, &c2, 2).unwrap();
        nap();
        snapshot_bounded(&root, &c1, 2).unwrap();
        nap();
        let s0 = snapshot_bounded(&root, &c0, 2).unwrap();
        assert!(s1.exists() && s0.exists());
        assert!(!s2.exists(), "least recently used snapshot pruned");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn rejects_option_like_revs_and_head_without_base() {
        let (repo, c1, _) = temp_repo("args");
        let root = repo.join("app");
        assert!(snapshot(&root, "--all").is_err());
        let e = impact_at(&root, None, Some(&c1), &no_scip(), &|_| {}).unwrap_err();
        assert!(e.to_string().contains("head requires base"), "{e}");
        assert!(
            !repo.join(".git/structure/snapshots").join(&c1).exists(),
            "no snapshot created"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn snapshots_pruned_to_bound() {
        let (repo, c1, c2) = temp_repo("prune");
        let root = repo.join("app");
        let s1 = snapshot_bounded(&root, &c1, 1).unwrap();
        let s2 = snapshot_bounded(&root, &c2, 1).unwrap();
        assert!(!s1.exists(), "oldest snapshot removed");
        assert!(s2.exists());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn impact_at_uses_that_commits_lines() {
        let (repo, c1, c2) = temp_repo("impact");
        let root = repo.join("app");
        // Line 2 is class B in c1 but class A in the working tree (c2).
        let base = format!("{c1}^");
        let imp = impact_at(&root, Some(&base), Some(&c1), &no_scip(), &|_| {}).unwrap();
        assert_eq!(imp.changed_symbols, vec!["B".to_string()], "{imp:?}");
        assert!(imp.warnings.iter().all(|w| !w.contains("checked out")));
        // Relative base resolves in the user's repo, not the snapshot's HEAD.
        let imp = impact_at(&root, Some("HEAD~2"), Some(&c1), &no_scip(), &|_| {}).unwrap();
        assert_eq!(imp.changed_symbols, vec!["B".to_string()], "{imp:?}");
        // c2 only touches the new comment line: no symbol changed.
        let imp = impact_at(&root, Some(&c1), Some(&c2), &no_scip(), &|_| {}).unwrap();
        assert!(imp.changed_symbols.is_empty(), "{imp:?}");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn gradle_settings_detected_as_java() {
        for f in ["settings.gradle", "settings.gradle.kts"] {
            let dir =
                std::env::temp_dir().join(format!("structure-detect-{}-{f}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(f), "").unwrap();
            assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn build_gradle_kts_detected_as_java() {
        let dir = std::env::temp_dir().join(format!("structure-detect-kts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("build.gradle.kts"), "").unwrap();
        assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_language_error_lists_gradle_files() {
        let dir = std::env::temp_dir().join(format!("structure-detect-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let msg = detect_language(&dir).unwrap_err().to_string();
        assert!(msg.contains("settings.gradle"), "{msg}");
        assert!(msg.contains("settings.gradle.kts"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gradle_dir_not_counted_as_stale() {
        let dir =
            std::env::temp_dir().join(format!("structure-gradle-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".gradle/x")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&index, t0);
        touch(&dir.join(".gradle/x/A.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 0);
        touch(&dir.join("B.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_index_detection_and_warning() {
        let dir = std::env::temp_dir().join(format!("structure-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&dir.join("src/old.ts"), t0);
        touch(&index, t0 + Duration::from_secs(10));
        touch(
            &dir.join("node_modules/dep.ts"),
            t0 + Duration::from_secs(50),
        );
        touch(&dir.join("src/notes.md"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 0);

        touch(&dir.join("src/new.ts"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 1);

        // Stale index reused with a warning when re-indexing is not wanted.
        let opts = Options {
            reindex_if_stale: false,
            ..Options::default()
        };
        let mut warnings = vec![];
        let got = ensure_index(&dir, Language::TypeScript, &opts, &mut |w| warnings.push(w));
        assert_eq!(got.unwrap(), Some(index));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("older than 1 source file"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
