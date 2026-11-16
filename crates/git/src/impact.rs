//! Diff impact analysis: changed lines from git, changed/impacted classes and cells.

use anyhow::{Context, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::Path;
use std::process::Command;
use structure_ir::ir::Graph;
use structure_ir::model::{Cell, Classification, Impact, ImpactedCell, ImpactedSymbol};

/// Changed line ranges of one file: 0-based, end-exclusive, NEW side.
/// An empty `lines` means the whole file is considered changed (deleted/renamed/binary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file: String,
    pub lines: Vec<(u32, u32)>,
}

pub fn git(project: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .output()
        .context("failed to run git")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Blob ids `git add` would give `paths` (relative to `project`) as they are
/// on disk, attribute filters (eol, clean) included.
pub fn hash_paths(project: &Path, paths: &[String]) -> anyhow::Result<Vec<String>> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    if paths.is_empty() {
        return Ok(vec![]);
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["hash-object", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to run git")?;
    let mut stdin = child.stdin.take().context("git stdin")?;
    // Absolute: `--stdin-paths` ignores the subdirectory git runs in.
    let mut input = String::new();
    for p in paths {
        input.push_str(&project.join(p).to_string_lossy());
        input.push('\n');
    }
    // Written from a thread: git answers while it reads, and a full stdout
    // pipe would otherwise block both sides.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut out = String::new();
    child
        .stdout
        .take()
        .context("git stdout")?
        .read_to_string(&mut out)?;
    let status = child.wait()?;
    let written = writer.join();
    if !status.success() {
        // Git's own message, not the broken pipe it caused on our side.
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        bail!("git hash-object failed: {}", err.trim());
    }
    written.map_err(|_| anyhow::anyhow!("git hash-object writer panicked"))??;
    let ids: Vec<String> = out.lines().map(str::to_string).collect();
    if ids.len() != paths.len() {
        bail!(
            "git hash-object returned {} ids for {} paths",
            ids.len(),
            paths.len()
        );
    }
    Ok(ids)
}

/// Changed lines (new side) between `base` and `head` / working tree, relative to `project`.
pub fn changed_lines(
    project: &Path,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Vec<FileChange>> {
    let mut args = vec![
        "diff",
        "--relative",
        "--no-color",
        "--no-ext-diff",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "-U0",
    ];
    match (base, head) {
        (None, Some(_)) => bail!("head requires base"),
        (None, None) => args.push("HEAD"),
        (Some(b), None) => args.push(b),
        (Some(b), Some(h)) => {
            args.push(b);
            args.push(h);
        }
    }
    let mut changes = parse_diff(&git(project, &args)?);

    if base.is_none() && head.is_none() {
        for rel in untracked(project)? {
            // The SCIP index written into the project is a build artefact
            // (often ~1 GB): never read it.
            if rel == SCIP_INDEX || rel.ends_with(&format!("/{SCIP_INDEX}")) {
                continue;
            }
            let total = line_count(&project.join(&rel)).unwrap_or(1).max(1);
            changes.push(FileChange {
                file: rel,
                lines: vec![(0, total)],
            });
        }
    }
    Ok(changes)
}

/// File name of the SCIP index the pipeline writes into a project root.
const SCIP_INDEX: &str = "index.scip";

/// Lines of a regular file (a last line without '\n' counts), read in fixed-size
/// chunks; `None` for anything else (symlink, directory, unreadable).
fn line_count(path: &Path) -> Option<u32> {
    use std::io::Read;
    if !std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file()) {
        return None;
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 64 * 1024];
    let (mut lines, mut last) = (0u32, b'\n');
    loop {
        let n = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        let chunk = &buf[..n];
        lines = lines.saturating_add(chunk.iter().filter(|&&c| c == b'\n').count() as u32);
        last = chunk[n - 1];
    }
    Some(lines + u32::from(last != b'\n'))
}

/// Untracked, non-ignored files under `project`, relative to it.
pub(crate) fn untracked(project: &Path) -> anyhow::Result<Vec<String>> {
    let prefix = git(project, &["rev-parse", "--show-prefix"])?;
    let prefix = prefix.trim();
    let listed = git(
        project,
        &["ls-files", "--others", "--exclude-standard", "--full-name"],
    )?;
    Ok(listed
        .lines()
        .filter_map(|p| p.strip_prefix(prefix))
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
}

/// Revisions go into the argv: one starting with '-' would be read as an option.
pub(crate) fn check_rev(rev: &str) -> anyhow::Result<()> {
    if rev.starts_with('-') {
        bail!("invalid revision {rev:?}");
    }
    Ok(())
}

/// Parse `git diff -U0` output into per-file changed ranges.
pub(crate) fn parse_diff(text: &str) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = Vec::new();
    let mut cur: Option<FileChange> = None;
    let mut old_path: Option<String> = None;
    let mut deleted = false;
    let mut in_hunk = false;
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            out.extend(cur.take());
            old_path = None;
            deleted = false;
            in_hunk = false;
            cur = Some(FileChange {
                file: String::new(),
                lines: vec![],
            });
        } else if in_hunk && !line.starts_with("@@ ") {
            // Hunk body: never a header, even if it starts with "-- " or "++ ".
        } else if let Some(p) = line.strip_prefix("rename to ") {
            if let Some(c) = cur.as_mut() {
                c.file = clean_path(p).to_string();
            }
        } else if let Some(p) = line.strip_prefix("--- ") {
            old_path = clean_path(p).strip_prefix("a/").map(str::to_string);
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if let Some(c) = cur.as_mut() {
                c.file = match clean_path(p).strip_prefix("b/") {
                    Some(n) => n.to_string(),
                    None => {
                        // /dev/null: deleted file, whole file counts as changed.
                        deleted = true;
                        old_path.clone().unwrap_or_default()
                    }
                };
            }
        } else if line.starts_with("@@ ") {
            in_hunk = true;
            if !deleted && let (Some(c), Some(range)) = (cur.as_mut(), parse_hunk(line)) {
                c.lines.push(range);
            }
        }
    }
    out.extend(cur);
    out.retain(|c| !c.file.is_empty());
    out
}

/// Strip a trailing tab (git adds one after paths with spaces) and surrounding quotes.
pub(crate) fn clean_path(p: &str) -> &str {
    let p = p.trim_end_matches('\t');
    p.strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .unwrap_or(p)
}

/// Changed lines of a unified diff TEXT (any number of context lines, e.g. the
/// 3 of a GitHub `.diff`), without running git. Walks hunk bodies, so context
/// lines are never reported. Same convention as [`FileChange`]: 0-based,
/// end-exclusive, NEW side; adjacent changed lines form one range; a block of
/// deletions with no added line maps to `(k, k + 1)`, `k` being the new-side
/// index of the line following it (what `-U0` parsing yields). Deleted files
/// have no lines; binary files and pure renames have empty `lines` (whole file).
/// Quoted (escaped) paths are not supported beyond surrounding quotes.
pub fn changes_from_diff(diff: &str) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = Vec::new();
    let mut cur: Option<FileChange> = None;
    let mut old_path: Option<String> = None;
    let mut deleted = false;
    // Hunk state: remaining old/new body lines, next new-side index, current
    // change block (start of its added run, position of its deletions).
    let (mut rem_old, mut rem_new) = (0u32, 0u32);
    let mut n = 0u32;
    let mut run: Option<u32> = None;
    let mut del_at: Option<u32> = None;
    fn flush(
        cur: &mut Option<FileChange>,
        deleted: bool,
        n: u32,
        run: &mut Option<u32>,
        del_at: &mut Option<u32>,
    ) {
        if let Some(c) = cur.as_mut().filter(|_| !deleted) {
            if let Some(s) = run.take() {
                c.lines.push((s, n));
            } else if let Some(k) = del_at.take() {
                c.lines.push((k, k + 1));
            }
        }
        *run = None;
        *del_at = None;
    }
    for line in diff.lines() {
        if rem_old > 0 || rem_new > 0 {
            match line.as_bytes().first() {
                Some(b'+') => {
                    run.get_or_insert(n);
                    n += 1;
                    rem_new = rem_new.saturating_sub(1);
                }
                Some(b'-') => {
                    del_at.get_or_insert(n);
                    rem_old = rem_old.saturating_sub(1);
                }
                Some(b'\\') => {}
                _ => {
                    flush(&mut cur, deleted, n, &mut run, &mut del_at);
                    n += 1;
                    rem_old = rem_old.saturating_sub(1);
                    rem_new = rem_new.saturating_sub(1);
                }
            }
            if rem_old == 0 && rem_new == 0 {
                flush(&mut cur, deleted, n, &mut run, &mut del_at);
            }
            continue;
        }
        if line.starts_with("diff --git ") {
            out.extend(cur.take());
            old_path = None;
            deleted = false;
            cur = Some(FileChange {
                file: String::new(),
                lines: vec![],
            });
        } else if let Some(p) = line.strip_prefix("rename to ") {
            if let Some(c) = cur.as_mut() {
                c.file = clean_path(p).to_string();
            }
        } else if let Some(p) = line.strip_prefix("--- ") {
            old_path = clean_path(p).strip_prefix("a/").map(str::to_string);
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if let Some(c) = cur.as_mut() {
                c.file = match clean_path(p).strip_prefix("b/") {
                    Some(f) => f.to_string(),
                    None => {
                        deleted = true;
                        old_path.clone().unwrap_or_default()
                    }
                };
            }
        } else if let Some(rest) = line
            .strip_prefix("Binary files ")
            .and_then(|r| r.strip_suffix(" differ"))
        {
            // "a/X and b/Y" (or /dev/null on one side): name the new path.
            if let Some(c) = cur.as_mut().filter(|c| c.file.is_empty())
                && let Some((a, b)) = rest.split_once(" and ")
            {
                let f = clean_path(b)
                    .strip_prefix("b/")
                    .or_else(|| clean_path(a).strip_prefix("a/"));
                c.file = f.unwrap_or_default().to_string();
            }
        } else if line.starts_with("@@ ")
            && let Some(((_, oc), (ns, nc))) = hunk_header(line)
        {
            rem_old = oc;
            rem_new = nc;
            n = if nc == 0 { ns } else { ns.saturating_sub(1) };
        }
    }
    flush(&mut cur, deleted, n, &mut run, &mut del_at);
    out.extend(cur);
    out.retain(|c| !c.file.is_empty());
    out
}

/// `@@ -os[,oc] +ns[,nc] @@` -> ((os, oc), (ns, nc)); a missing count is 1.
fn hunk_header(line: &str) -> Option<((u32, u32), (u32, u32))> {
    fn side(t: &str) -> Option<(u32, u32)> {
        match t.split_once(',') {
            Some((s, c)) => Some((s.parse().ok()?, c.parse().ok()?)),
            None => Some((t.parse().ok()?, 1)),
        }
    }
    let mut it = line.split_whitespace().skip(1);
    let old = side(it.next()?.strip_prefix('-')?)?;
    let new = side(it.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

fn parse_hunk(line: &str) -> Option<(u32, u32)> {
    let new = line.split_whitespace().find(|t| t.starts_with('+'))?;
    let new = &new[1..];
    let (start, count) = match new.split_once(',') {
        Some((s, c)) => (s.parse::<u32>().ok()?, c.parse::<u32>().ok()?),
        None => (new.parse::<u32>().ok()?, 1),
    };
    if count == 0 {
        Some((start, start + 1))
    } else {
        Some((start.saturating_sub(1), start.saturating_sub(1) + count))
    }
}

pub fn analyze(
    project: &Path,
    graph: &Graph,
    cls: &Classification,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Impact> {
    let changes = changed_lines(project, base, head)?;
    Ok(compute(graph, cls, &changes, base.unwrap_or("HEAD"), head))
}

/// What [`compute`] needs from a graph, independent of the diff: built once per
/// analysis and reused by every impact on it. Symbols are indices into
/// `Graph::symbols`; on duplicate ids the last symbol stands for the id.
#[derive(Debug, Clone, Default)]
pub struct ImpactIndex {
    /// Top-level ancestor of each symbol's id; `None` on a container cycle.
    top: Vec<Option<u32>>,
    /// Symbols of each file, in graph order.
    by_file: HashMap<String, Vec<u32>>,
    /// `callers[caller_start[t]..caller_start[t + 1]]`: top-level symbols with
    /// an edge to top-level symbol `t` (deduplicated, never `t` itself).
    caller_start: Vec<u32>,
    callers: Vec<u32>,
}

impl ImpactIndex {
    pub fn new(graph: &Graph) -> ImpactIndex {
        let n = graph.symbols.len();
        let by_id: HashMap<&str, u32> = graph
            .symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i as u32))
            .collect();
        let lift = |id: &str| -> Option<u32> {
            let mut cur = *by_id.get(id)?;
            for _ in 0..=n {
                match graph.symbols[cur as usize]
                    .container
                    .as_deref()
                    .and_then(|c| by_id.get(c))
                {
                    Some(&p) => cur = p,
                    None => return Some(cur),
                }
            }
            None
        };
        let top: Vec<Option<u32>> = graph.symbols.iter().map(|s| lift(&s.id)).collect();
        let top_of = |id: &str| by_id.get(id).and_then(|&i| top[i as usize]);

        let mut by_file: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, s) in graph.symbols.iter().enumerate() {
            match by_file.get_mut(s.file.as_str()) {
                Some(v) => v.push(i as u32),
                None => {
                    by_file.insert(s.file.clone(), vec![i as u32]);
                }
            }
        }

        // (callee, caller) pairs, sorted and deduplicated, then laid out per callee.
        let mut pairs: Vec<(u32, u32)> = graph
            .edges
            .iter()
            .filter_map(|e| match (top_of(&e.from), top_of(&e.to)) {
                (Some(f), Some(t)) if f != t => Some((t, f)),
                _ => None,
            })
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        let mut caller_start = vec![0u32; n + 1];
        for &(t, _) in &pairs {
            caller_start[t as usize + 1] += 1;
        }
        for i in 0..n {
            caller_start[i + 1] += caller_start[i];
        }
        let callers = pairs.into_iter().map(|(_, f)| f).collect();
        ImpactIndex {
            top,
            by_file,
            caller_start,
            callers,
        }
    }

    fn callers_of(&self, t: u32) -> &[u32] {
        let (a, b) = (
            self.caller_start[t as usize],
            self.caller_start[t as usize + 1],
        );
        &self.callers[a as usize..b as usize]
    }
}

/// [`compute_with`] on an index built for this call only.
pub fn compute(
    graph: &Graph,
    cls: &Classification,
    changes: &[FileChange],
    base: &str,
    head: Option<&str>,
) -> Impact {
    compute_with(graph, cls, &ImpactIndex::new(graph), changes, base, head)
}

/// Impact of `changes` on `graph`; `index` must have been built from `graph`.
/// The composition of `resolve`, `propagate` and `aggregate`.
pub fn compute_with(
    graph: &Graph,
    cls: &Classification,
    index: &ImpactIndex,
    changes: &[FileChange],
    base: &str,
    head: Option<&str>,
) -> Impact {
    let id = |i: u32| graph.symbols[i as usize].id.as_str();

    let mut changed_files: Vec<String> = changes.iter().map(|c| c.file.clone()).collect();
    changed_files.sort();
    changed_files.dedup();

    let changed = resolve(graph, index, changes);
    let mut impacted_symbols: Vec<ImpactedSymbol> = propagate(index, &changed)
        .into_iter()
        .map(|(i, distance)| ImpactedSymbol {
            id: id(i).to_string(),
            distance,
        })
        .collect();
    impacted_symbols.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
    let mut changed_symbols: Vec<String> = changed.iter().map(|&i| id(i).to_string()).collect();
    changed_symbols.sort();

    let CellImpact {
        changed: changed_cells,
        impacted: impacted_cells,
    } = aggregate(cls, &changed_symbols, &impacted_symbols);

    Impact {
        base: base.to_string(),
        head: head.map(str::to_string),
        changed_files,
        changed_symbols,
        changed_cells,
        impacted_cells,
        impacted_symbols,
        warnings: vec![],
        matrix: None,
    }
}

/// Resolve step: the top-level symbols (indices into `graph.symbols`) whose
/// range overlaps a changed line range. A change with no lines, or a range
/// outside every symbol of its file (package, imports, file comments), marks
/// every symbol of the file. Files the graph does not know mark nothing.
fn resolve(graph: &Graph, index: &ImpactIndex, changes: &[FileChange]) -> BTreeSet<u32> {
    let mut changed: BTreeSet<u32> = BTreeSet::new();
    for ch in changes {
        let spans: Vec<(u32, u32, u32)> = index
            .by_file
            .get(ch.file.as_str())
            .into_iter()
            .flatten()
            .map(|&i| {
                let r = &graph.symbols[i as usize].range;
                let st = r.start_line;
                let en = (r.end_line + u32::from(r.end_col > 0)).max(st + 1);
                (i, st, en)
            })
            .collect();
        let whole = ch.lines.is_empty()
            || ch
                .lines
                .iter()
                .any(|&(a, b)| !spans.iter().any(|&(_, st, en)| st < b && a < en));
        for &(i, st, en) in &spans {
            if whole || ch.lines.iter().any(|&(a, b)| st < b && a < en) {
                changed.extend(index.top[i as usize]);
            }
        }
    }
    changed
}

/// Propagate step: breadth-first over callers of the top-level `seeds`. Returns
/// every reached top-level symbol that is not a seed, with its shortest
/// distance (>= 1) from any seed.
fn propagate(index: &ImpactIndex, seeds: &BTreeSet<u32>) -> HashMap<u32, u32> {
    let mut dist: HashMap<u32, u32> = seeds.iter().map(|&c| (c, 0)).collect();
    let mut queue: VecDeque<(u32, u32)> = seeds.iter().map(|&c| (c, 0)).collect();
    while let Some((t, d)) = queue.pop_front() {
        for &caller in index.callers_of(t) {
            if let std::collections::hash_map::Entry::Vacant(e) = dist.entry(caller) {
                e.insert(d + 1);
                queue.push_back((caller, d + 1));
            }
        }
    }
    dist.retain(|i, _| !seeds.contains(i));
    dist
}

/// Cells touched by an impact, as [`aggregate`] computes them.
#[derive(Debug, Clone, PartialEq)]
struct CellImpact {
    /// Cells of the changed symbols, sorted.
    changed: Vec<String>,
    /// Other cells of impacted symbols, with the smallest distance of their
    /// symbols, sorted by (distance, id).
    impacted: Vec<ImpactedCell>,
}

/// Aggregate step: symbols to `layer:domain` cells through the classification.
/// Unplaced symbols have no cell; a changed cell is never also impacted.
fn aggregate(
    cls: &Classification,
    changed_symbols: &[String],
    impacted_symbols: &[ImpactedSymbol],
) -> CellImpact {
    let cell_of = |id: &str| {
        cls.placements
            .get(id)
            .map(|p| Cell::id_for(p.layer, &p.domain))
    };
    let changed: BTreeSet<String> = changed_symbols.iter().filter_map(|s| cell_of(s)).collect();
    let mut cell_dist: BTreeMap<String, u32> = BTreeMap::new();
    for s in impacted_symbols {
        if let Some(c) = cell_of(&s.id)
            && !changed.contains(&c)
        {
            let e = cell_dist.entry(c).or_insert(s.distance);
            *e = (*e).min(s.distance);
        }
    }
    let mut impacted: Vec<ImpactedCell> = cell_dist
        .into_iter()
        .map(|(id, distance)| ImpactedCell { id, distance })
        .collect();
    impacted.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
    CellImpact {
        changed: changed.into_iter().collect(),
        impacted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use structure_ir::ir::{Edge, EdgeKind, Language, Range, Symbol, SymbolKind};
    use structure_ir::model::{Layer, Placement};

    #[test]
    fn parse_modify_and_multiple_hunks() {
        let d = "diff --git a/A.java b/A.java\nindex 1..2 100644\n--- a/A.java\n+++ b/A.java\n@@ -3 +3 @@ x\n-a\n+b\n@@ -10,2 +10,3 @@\n";
        let v = parse_diff(d);
        assert_eq!(
            v,
            vec![FileChange {
                file: "A.java".into(),
                lines: vec![(2, 3), (9, 12)]
            }]
        );
    }

    #[test]
    fn parse_ignores_header_lookalikes_in_hunks() {
        let d =
            "diff --git a/A b/A\n--- a/A\n+++ b/A\n@@ -1,2 +1,2 @@\n--- x\n+++ y\n@@ -9 +9 @@\n";
        let v = parse_diff(d);
        assert_eq!(
            v,
            vec![FileChange {
                file: "A".into(),
                lines: vec![(0, 2), (8, 9)]
            }]
        );
    }

    #[test]
    fn parse_add_delete_only_newfile_rename() {
        let d = "diff --git a/A b/A\n--- a/A\n+++ b/A\n@@ -5,0 +6,2 @@\n\
diff --git a/B b/B\n--- a/B\n+++ b/B\n@@ -4,2 +3,0 @@\n\
diff --git a/N b/N\nnew file mode 100644\n--- /dev/null\n+++ b/N\n@@ -0,0 +1,4 @@\n\
diff --git a/Old b/New\nsimilarity index 90%\nrename from Old\nrename to New\n--- a/Old\n+++ b/New\n@@ -1 +1 @@\n\
diff --git a/D b/D\ndeleted file mode 100644\n--- a/D\n+++ /dev/null\n@@ -1,3 +0,0 @@\n";
        let v = parse_diff(d);
        assert_eq!(v[0].lines, vec![(5, 7)]);
        assert_eq!(v[1].lines, vec![(3, 4)]);
        assert_eq!(
            (v[2].file.as_str(), v[2].lines.clone()),
            ("N", vec![(0, 4)])
        );
        assert_eq!(
            (v[3].file.as_str(), v[3].lines.clone()),
            ("New", vec![(0, 1)])
        );
        assert_eq!(v[4].file, "D");
        assert!(v[4].lines.is_empty());
    }

    #[test]
    fn diff_text_u3_equals_u0() {
        let u3 = "diff --git a/A b/A\n--- a/A\n+++ b/A\n@@ -2,7 +2,8 @@\n c1\n c2\n c3\n-old\n+new\n+new2\n c4\n c5\n c6\n@@ -20,7 +21,6 @@\n d1\n d2\n d3\n-gone\n d4\n d5\n d6\n";
        let u0 = "diff --git a/A b/A\n--- a/A\n+++ b/A\n@@ -5 +5,2 @@\n-old\n+new\n+new2\n@@ -23 +23,0 @@\n-gone\n";
        let expect = vec![FileChange {
            file: "A".into(),
            lines: vec![(4, 6), (23, 24)],
        }];
        assert_eq!(changes_from_diff(u3), expect);
        assert_eq!(changes_from_diff(u0), expect);
        assert_eq!(parse_diff(u0), expect);
    }

    #[test]
    fn diff_text_added_deleted_renamed_binary_multi() {
        let d = "diff --git a/N b/N\nnew file mode 100644\n--- /dev/null\n+++ b/N\n@@ -0,0 +1,2 @@\n+a\n+b\n\\ No newline at end of file\n\
diff --git a/D b/D\ndeleted file mode 100644\n--- a/D\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n\
diff --git a/Old b/New\nsimilarity index 100%\nrename from Old\nrename to New\n\
diff --git a/Old2 b/New2\nsimilarity index 80%\nrename from Old2\nrename to New2\n--- a/Old2\n+++ b/New2\n@@ -1,3 +1,3 @@\n x\n-y\n+z\n w\n\
diff --git a/i.png b/i.png\nindex 1..2 100644\nBinary files a/i.png and b/i.png differ\n\
diff --git a/M b/M\n--- a/M\n+++ b/M\n@@ -1,2 +1,2 @@\n-a\n\\ No newline at end of file\n+b\n\\ No newline at end of file\n c\n";
        let v = changes_from_diff(d);
        let got: Vec<(&str, Vec<(u32, u32)>)> = v
            .iter()
            .map(|c| (c.file.as_str(), c.lines.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("N", vec![(0, 2)]),
                ("D", vec![]),
                ("New", vec![]),
                ("New2", vec![(1, 2)]),
                ("i.png", vec![]),
                ("M", vec![(0, 1)]),
            ]
        );
    }

    #[test]
    fn diff_text_context_not_changed_and_adjacent_merge() {
        let d = "diff --git a/A b/A\n--- a/A\n+++ b/A\n@@ -1,4 +1,7 @@\n a\n+b\n+c\n-d\n+e\n f\n+g\n h\n";
        assert_eq!(changes_from_diff(d)[0].lines, vec![(1, 4), (5, 6)]);
    }

    fn sym(id: &str, container: Option<&str>, file: &str, s: u32, e: u32) -> Symbol {
        let r = Range {
            start_line: s,
            start_col: 0,
            end_line: e,
            end_col: 1,
        };
        Symbol {
            id: id.into(),
            kind: if container.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Class
            },
            name: id.into(),
            container: container.map(Into::into),
            file: file.into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
            package: None,
        }
    }

    fn edge(f: &str, t: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: f.into(),
            to: t.into(),
            kind,
        }
    }

    fn fixture() -> (Graph, Classification) {
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym("Mapper", None, "Mapper.java", 0, 20),
                sym("Mapper#map", Some("Mapper"), "Mapper.java", 5, 10),
                sym("Mapper#other", Some("Mapper"), "Mapper.java", 11, 15),
                sym("Service", None, "Service.java", 0, 20),
                sym("Service#run", Some("Service"), "Service.java", 3, 8),
                sym("Action", None, "Action.java", 0, 10),
                sym("Iface", None, "Iface.java", 0, 5),
                sym("Impl", None, "Impl.java", 0, 5),
                sym("User", None, "User.java", 0, 5),
                sym("Lonely", None, "Lonely.java", 0, 5),
            ],
            edges: vec![
                edge("Service#run", "Mapper#map", EdgeKind::Call),
                edge("Action", "Service", EdgeKind::Uses),
                edge("Impl", "Iface", EdgeKind::Implements),
                edge("User", "Iface", EdgeKind::Uses),
                edge("Mapper#map", "Mapper#other", EdgeKind::Call),
                edge("Action", "ext:foo", EdgeKind::Call),
            ],
        };
        let mut c = Classification::default();
        for (id, l, d) in [
            ("Mapper", Layer::Persistence, "a"),
            ("Service", Layer::Business, "a"),
            ("Action", Layer::Entry, "a"),
            ("Iface", Layer::Business, "b"),
            ("Impl", Layer::Business, "b"),
        ] {
            c.placements.insert(
                id.into(),
                Placement {
                    layer: l,
                    domain: d.into(),
                },
            );
        }
        (g, c)
    }

    #[test]
    fn chain_distances_and_cells() {
        let (g, c) = fixture();
        let ch = [FileChange {
            file: "Mapper.java".into(),
            lines: vec![(6, 7)],
        }];
        let i = compute(&g, &c, &ch, "HEAD", None);
        assert_eq!(i.changed_files, ["Mapper.java"]);
        assert_eq!(i.changed_symbols, ["Mapper"]);
        assert_eq!(i.changed_cells, ["persistence:a"]);
        let ids: Vec<_> = i
            .impacted_symbols
            .iter()
            .map(|s| (s.id.as_str(), s.distance))
            .collect();
        assert_eq!(ids, [("Service", 1), ("Action", 2)]);
        let cells: Vec<_> = i
            .impacted_cells
            .iter()
            .map(|s| (s.id.as_str(), s.distance))
            .collect();
        assert_eq!(cells, [("business:a", 1), ("entry:a", 2)]);
    }

    #[test]
    fn interface_implementations_and_no_hit() {
        let (g, c) = fixture();
        let ch = [FileChange {
            file: "Iface.java".into(),
            lines: vec![(0, 1)],
        }];
        let i = compute(&g, &c, &ch, "x", Some("y"));
        assert_eq!(i.head.as_deref(), Some("y"));
        let ids: Vec<_> = i.impacted_symbols.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["Impl", "User"]);
        // Impl shares the changed cell business:b; User has no placement.
        assert!(i.impacted_cells.is_empty());
        // A file the graph does not know (e.g. a test) marks nothing.
        let none = [FileChange {
            file: "MapperTest.java".into(),
            lines: vec![(0, 1)],
        }];
        assert!(
            compute(&g, &c, &none, "HEAD", None)
                .changed_symbols
                .is_empty()
        );
    }

    #[test]
    fn header_change_marks_the_file_symbols() {
        let mut g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                // package + imports on lines 0..4, two top-level classes.
                sym("Api", None, "Api.java", 4, 20),
                sym("Api#get", Some("Api"), "Api.java", 6, 10),
                sym("Param", None, "Api.java", 22, 30),
                sym("Other", None, "Other.java", 0, 5),
            ],
            edges: vec![],
        };
        g.edges.push(edge("Other", "Param", EdgeKind::Uses));
        let c = Classification::default();
        let import = [FileChange {
            file: "Api.java".into(),
            lines: vec![(2, 3)],
        }];
        let i = compute(&g, &c, &import, "HEAD", None);
        assert_eq!(i.changed_symbols, ["Api", "Param"]);
        assert_eq!(i.impacted_symbols[0].id, "Other");
        // Between the two classes, too.
        let gap = [FileChange {
            file: "Api.java".into(),
            lines: vec![(21, 22)],
        }];
        assert_eq!(
            compute(&g, &c, &gap, "HEAD", None).changed_symbols,
            ["Api", "Param"]
        );
        // Inside a method: only its class.
        let body = [FileChange {
            file: "Api.java".into(),
            lines: vec![(7, 8)],
        }];
        assert_eq!(
            compute(&g, &c, &body, "HEAD", None).changed_symbols,
            ["Api"]
        );
    }

    #[test]
    fn deleted_file_marks_all_symbols() {
        let (g, c) = fixture();
        let ch = [FileChange {
            file: "Service.java".into(),
            lines: vec![],
        }];
        let i = compute(&g, &c, &ch, "HEAD", None);
        assert_eq!(i.changed_symbols, ["Service"]);
        assert_eq!(i.impacted_symbols[0].id, "Action");
    }

    #[test]
    fn reused_index_handles_duplicate_ids_and_container_cycles() {
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym("A", None, "A.java", 0, 10),
                sym("A#m", Some("A"), "A.java", 2, 4),
                // Same id twice: the last one stands for `B`.
                sym("B", None, "B.java", 0, 5),
                sym("B", None, "B2.java", 0, 5),
                // Container cycle: never lifted.
                sym("X", Some("Y"), "X.java", 0, 5),
                sym("Y", Some("X"), "X.java", 6, 9),
            ],
            edges: vec![
                edge("B", "A#m", EdgeKind::Call),
                edge("B", "A", EdgeKind::Uses),
                edge("X", "A", EdgeKind::Uses),
                edge("A#m", "A", EdgeKind::Uses),
            ],
        };
        let c = Classification::default();
        let index = ImpactIndex::new(&g);
        let ch = |f: &str| {
            [FileChange {
                file: f.into(),
                lines: vec![],
            }]
        };
        for f in ["A.java", "B.java", "B2.java", "X.java"] {
            assert_eq!(
                compute_with(&g, &c, &index, &ch(f), "HEAD", None),
                compute(&g, &c, &ch(f), "HEAD", None),
                "{f}"
            );
        }
        let i = compute_with(&g, &c, &index, &ch("A.java"), "HEAD", None);
        assert_eq!(i.changed_symbols, ["A"]);
        let ids: Vec<_> = i
            .impacted_symbols
            .iter()
            .map(|s| (s.id.as_str(), s.distance))
            .collect();
        assert_eq!(ids, [("B", 1)]);
        assert_eq!(
            compute_with(&g, &c, &index, &ch("B.java"), "HEAD", None).changed_symbols,
            ["B"]
        );
        assert!(
            compute_with(&g, &c, &index, &ch("X.java"), "HEAD", None)
                .changed_symbols
                .is_empty()
        );
    }

    /// Indices of `ids` in `g.symbols` (first match).
    fn idx(g: &Graph, ids: &[&str]) -> BTreeSet<u32> {
        ids.iter()
            .filter_map(|id| g.symbols.iter().position(|s| s.id == *id))
            .map(|i| i as u32)
            .collect()
    }

    #[test]
    fn resolve_lifts_overlaps_to_top_level() {
        let (g, _) = fixture();
        let index = ImpactIndex::new(&g);
        let fc = |file: &str, lines: Vec<(u32, u32)>| FileChange {
            file: file.into(),
            lines,
        };
        // A method body: its class only.
        assert_eq!(
            resolve(&g, &index, &[fc("Mapper.java", vec![(6, 7)])]),
            idx(&g, &["Mapper"])
        );
        // Several files at once; an unknown file marks nothing.
        assert_eq!(
            resolve(
                &g,
                &index,
                &[
                    fc("Service.java", vec![(4, 5)]),
                    fc("Iface.java", vec![]),
                    fc("Nope.java", vec![(0, 1)]),
                ]
            ),
            idx(&g, &["Service", "Iface"])
        );
        // A range past every symbol marks the whole file.
        assert_eq!(
            resolve(&g, &index, &[fc("Action.java", vec![(40, 41)])]),
            idx(&g, &["Action"])
        );
        assert!(resolve(&g, &index, &[]).is_empty());
    }

    #[test]
    fn propagate_returns_non_seed_distances() {
        let (g, _) = fixture();
        let index = ImpactIndex::new(&g);
        let by_id = |m: HashMap<u32, u32>| -> BTreeMap<String, u32> {
            m.into_iter()
                .map(|(i, d)| (g.symbols[i as usize].id.clone(), d))
                .collect()
        };
        let got = by_id(propagate(&index, &idx(&g, &["Mapper"])));
        assert_eq!(
            got,
            BTreeMap::from([("Service".into(), 1), ("Action".into(), 2)])
        );
        // A seed reached from another seed stays out; distances are shortest.
        let got = by_id(propagate(&index, &idx(&g, &["Mapper", "Service"])));
        assert_eq!(got, BTreeMap::from([("Action".into(), 1)]));
        // No callers, no seeds: nothing.
        assert!(propagate(&index, &idx(&g, &["Lonely"])).is_empty());
        assert!(propagate(&index, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn aggregate_maps_symbols_to_cells() {
        let (_, c) = fixture();
        let imp = |id: &str, distance: u32| ImpactedSymbol {
            id: id.into(),
            distance,
        };
        let got = aggregate(
            &c,
            &["Mapper".into(), "Iface".into(), "Lonely".into()],
            &[
                imp("Action", 3),
                imp("Service", 2),
                imp("Impl", 1),
                imp("User", 1),
                imp("Service", 1),
            ],
        );
        assert_eq!(got.changed, ["business:b", "persistence:a"]);
        // Impl shares a changed cell; User is unplaced; the min distance wins.
        let cells: Vec<_> = got
            .impacted
            .iter()
            .map(|s| (s.id.as_str(), s.distance))
            .collect();
        assert_eq!(cells, [("business:a", 1), ("entry:a", 3)]);
        assert_eq!(
            aggregate(&c, &[], &[]),
            CellImpact {
                changed: vec![],
                impacted: vec![],
            }
        );
    }
}
