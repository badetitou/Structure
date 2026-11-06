use std::path::Path;

use anyhow::{Context, bail};

use crate::impact;
use crate::model::{Commit, History, Impact};

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
