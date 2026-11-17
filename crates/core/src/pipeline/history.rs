use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::impact::{self, FileChange};
use crate::model::{Commit, History, Impact};
use structure_git::is_scip_index;

use super::Analysis;

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
            !is_scip_index(p)
        });
    Ok(History { dirty, commits })
}

pub fn impact(
    root: &Path,
    analysis: &Analysis,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Impact> {
    let changes = impact::changed_lines(root, base, head)?;
    let mut result = compute(root, analysis, changes, base.unwrap_or("HEAD"), head);
    if let Some(h) = head {
        result.warnings.push(format!(
            "symbol positions come from the working tree; results for {h} are exact only if it is checked out"
        ));
    }
    Ok(result)
}

/// Impact of `changes` (git paths relative to `root`) on `analysis`. The SCIP
/// indexes we write into the project are build artefacts, not changes. In a
/// merged analysis the graph names files `<project>/<file>`: each change is
/// mapped to every project whose root is `root` or below it (a root holding
/// several languages has one project per language); `changed_files` keeps
/// the git paths. Carries the analysis' warnings.
pub(super) fn compute(
    root: &Path,
    analysis: &Analysis,
    mut changes: Vec<FileChange>,
    base: &str,
    head: Option<&str>,
) -> Impact {
    changes.retain(|c| !is_scip_index(&c.file));
    let mapped = unit_changes(root, analysis, &changes);
    let mut result = impact::compute_with(
        &analysis.graph,
        &analysis.classification,
        analysis.impact_index(),
        mapped.as_deref().unwrap_or(&changes),
        base,
        head,
    );
    if mapped.is_some() {
        let mut files: Vec<String> = changes.into_iter().map(|c| c.file).collect();
        files.sort();
        files.dedup();
        result.changed_files = files;
    }
    result.warnings = analysis.warnings.clone();
    result
}

/// `changes` with the graph's file names of a merged analysis, `None` for a
/// single-project one (its files are the git paths already).
fn unit_changes(root: &Path, a: &Analysis, changes: &[FileChange]) -> Option<Vec<FileChange>> {
    let projects = &a.matrix.projects;
    if projects.is_empty() || projects.len() != a.roots.len() {
        return None;
    }
    let canon = |p: &Path| -> PathBuf { p.canonicalize().unwrap_or_else(|_| p.to_path_buf()) };
    let root = canon(root);
    let mut out = vec![];
    for (unit_root, p) in a.roots.iter().zip(projects) {
        let Ok(rel) = canon(unit_root).strip_prefix(&root).map(Path::to_path_buf) else {
            continue;
        };
        let rel: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let dir = rel.join("/");
        for c in changes {
            let file = if dir.is_empty() {
                c.file.as_str()
            } else {
                match c.file.strip_prefix(&dir).and_then(|f| f.strip_prefix('/')) {
                    Some(f) => f,
                    None => continue,
                }
            };
            out.push(FileChange {
                file: format!("{}/{file}", p.name),
                lines: c.lines.clone(),
            });
        }
    }
    Some(out)
}
