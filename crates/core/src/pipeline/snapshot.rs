use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::impact;
use crate::model::{Impact, Progress};

use super::{Analysis, Options, analyze, analyze_as, project_name};
use structure_git::resolve_base;

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
    let snaps = super::structure_dir(root)?.join("snapshots");
    std::fs::create_dir_all(&snaps).with_context(|| format!("creating {}", snaps.display()))?;
    let snaps = snaps.canonicalize().unwrap_or(snaps);
    let dir = snaps.join(&sha);
    let dir_str = dir.to_string_lossy().into_owned();

    let valid = dir.join(".git").exists()
        && impact::git(&dir, &["rev-parse", "--show-toplevel"])
            .is_ok_and(|t| Path::new(t.trim()).canonicalize().ok() == dir.canonicalize().ok())
        && impact::git(&dir, &["rev-parse", "HEAD"]).is_ok_and(|h| h.trim() == sha)
        // Tracked files deleted from the checkout; untracked ones (SCIP indexes, builds) are fine.
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

/// Snapshot of `head` (see [`snapshot`]) and its analysis. A commit never
/// changes, so callers may cache the result per (project, head). The
/// snapshot is analysed as the working tree at `root` is by
/// [`super::analyze_many`]: an undetectable root is expanded to its
/// sub-projects, and projects are named after `root`, not after the
/// snapshot's directory (the commit SHA when `root` is the repository root),
/// so ids and cells match the working tree's.
pub fn analyze_snapshot(
    root: &Path,
    head: &str,
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<(PathBuf, Analysis)> {
    let snap = snapshot(root, head)?;
    let analysis = analyze_as(&snap, &project_name(root), opts, progress, !opts.exact)?;
    Ok((snap, analysis))
}

/// Impact of `base..head` on the analysis of `head`'s snapshot `snap`; the
/// result carries that analysis' matrix.
pub fn impact_in_snapshot(
    snap: &Path,
    analysis: &Analysis,
    base_sha: Option<&str>,
    head: &str,
) -> anyhow::Result<Impact> {
    let changes = impact::changed_lines(snap, base_sha, Some(head))?;
    let mut result = super::history::compute(
        snap,
        analysis,
        changes,
        base_sha.unwrap_or("HEAD"),
        Some(head),
    );
    result.matrix = Some(analysis.matrix.clone());
    Ok(result)
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
        return super::history::impact(root, &analysis, base, None);
    };
    let base_sha = base.map(|b| resolve_base(root, b)).transpose()?;
    let (snap, analysis) = analyze_snapshot(root, h, opts, progress)?;
    impact_in_snapshot(&snap, &analysis, base_sha.as_deref(), h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_support::*;
    use std::time::Duration;

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
        assert!(imp.matrix.is_some(), "snapshot path carries its matrix");
        // Relative base resolves in the user's repo, not the snapshot's HEAD.
        let imp = impact_at(&root, Some("HEAD~2"), Some(&c1), &no_scip(), &|_| {}).unwrap();
        assert_eq!(imp.changed_symbols, vec!["B".to_string()], "{imp:?}");
        // c2 only adds a header comment: outside every class, so it marks the
        // whole file (with c1's positions, line 0 would be class A alone).
        let imp = impact_at(&root, Some(&c1), Some(&c2), &no_scip(), &|_| {}).unwrap();
        assert_eq!(imp.changed_symbols, ["A", "B"], "{imp:?}");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn impact_at_root_commit_with_empty_tree_base() {
        let (repo, _, _) = temp_repo("root");
        let root = repo.join("app");
        let c0 = git_ok(&repo, &["rev-list", "--max-parents=0", "HEAD"])
            .trim()
            .to_string();
        let empty = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
        let imp = impact_at(&root, Some(empty), Some(&c0), &no_scip(), &|_| {}).unwrap();
        assert_eq!(imp.changed_symbols, vec!["A".to_string()], "{imp:?}");
        let _ = std::fs::remove_dir_all(&repo);
    }
}
