use std::path::Path;

use anyhow::Context;

use crate::impact;

/// Full SHA (or tree id) of `base`; a root commit's diff base is the empty tree.
pub fn resolve_base(root: &Path, base: &str) -> anyhow::Result<String> {
    let resolve = |kind: &str| {
        impact::git(
            root,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{base}^{{{kind}}}"),
            ],
        )
    };
    let sha = resolve("commit")
        .or_else(|_| resolve("tree"))
        .with_context(|| format!("unknown revision: {base}"))?;
    Ok(sha.trim().to_string())
}

/// Full SHA of the commit `rev` names (branch, short SHA, `HEAD~2`...), so
/// callers can key caches on one identity.
pub fn resolve_commit(root: &Path, rev: &str) -> anyhow::Result<String> {
    let sha = impact::git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .with_context(|| format!("unknown revision: {rev}"))?;
    Ok(sha.trim().to_string())
}

/// Identifies the state of the working tree: HEAD, `git status` and the size
/// and mtime of every listed file. Equal fingerprints mean a cached working-tree
/// analysis is still valid.
pub fn worktree_fingerprint(root: &Path) -> anyhow::Result<String> {
    let head = impact::git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let top = impact::git(root, &["rev-parse", "--show-toplevel"])?;
    let top = Path::new(top.trim());
    // Paths are relative to the repository root; untracked files one by one.
    let status = impact::git(
        root,
        &[
            "status",
            "--porcelain",
            "-z",
            "--no-renames",
            "-uall",
            "--",
            ".",
        ],
    )?;
    let mut fp = format!("{}\n", head.trim());
    for entry in status.split('\0').filter(|e| e.len() > 3) {
        let path = entry.get(3..).unwrap_or("");
        let meta = std::fs::metadata(top.join(path)).ok();
        let (len, mtime) = meta.map_or((0, 0), |m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (m.len(), t)
        });
        fp.push_str(&format!("{entry}\t{len}\t{mtime}\n"));
    }
    Ok(fp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_support::*;

    #[test]
    fn resolve_commit_gives_full_sha() {
        let (repo, c1, _) = temp_repo("resolve");
        let root = repo.join("app");
        assert_eq!(resolve_commit(&root, &c1[..8]).unwrap(), c1);
        assert_eq!(resolve_commit(&root, "HEAD~1").unwrap(), c1);
        assert!(resolve_commit(&root, "nope").is_err());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn worktree_fingerprint_tracks_edits() {
        let (repo, _, _) = temp_repo("fingerprint");
        let root = repo.join("app");
        let clean = worktree_fingerprint(&root).unwrap();
        assert_eq!(clean, worktree_fingerprint(&root).unwrap());
        std::fs::write(root.join("New.java"), "class N {}\n").unwrap();
        let dirty = worktree_fingerprint(&root).unwrap();
        assert_ne!(clean, dirty);
        std::fs::write(root.join("New.java"), "class N { int x; }\n").unwrap();
        assert_ne!(dirty, worktree_fingerprint(&root).unwrap());
        let _ = std::fs::remove_dir_all(&repo);
    }
}
