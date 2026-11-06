//! Diff impact analysis: changed lines from git, changed/impacted classes and cells.

use crate::ir::{Graph, Symbol};
use crate::model::{Cell, Classification, Impact, ImpactedCell, ImpactedSymbol};
use anyhow::{Context, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::Path;
use std::process::Command;

/// Changed line ranges of one file: 0-based, end-exclusive, NEW side.
/// An empty `lines` means the whole file is considered changed (deleted/renamed/binary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file: String,
    pub lines: Vec<(u32, u32)>,
}

pub(crate) fn git(project: &Path, args: &[&str]) -> anyhow::Result<String> {
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
        let prefix = git(project, &["rev-parse", "--show-prefix"])?;
        let prefix = prefix.trim();
        let listed = git(
            project,
            &["ls-files", "--others", "--exclude-standard", "--full-name"],
        )?;
        for path in listed.lines().filter(|l| !l.is_empty()) {
            let Some(rel) = path.strip_prefix(prefix) else {
                continue;
            };
            let total = std::fs::read(project.join(rel))
                .map(|b| {
                    let nl = b.iter().filter(|&&c| c == b'\n').count() as u32;
                    nl + u32::from(b.last().is_some_and(|&c| c != b'\n'))
                })
                .unwrap_or(1)
                .max(1);
            changes.push(FileChange {
                file: rel.to_string(),
                lines: vec![(0, total)],
            });
        }
    }
    Ok(changes)
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
fn clean_path(p: &str) -> &str {
    let p = p.trim_end_matches('\t');
    p.strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .unwrap_or(p)
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

pub fn compute(
    graph: &Graph,
    cls: &Classification,
    changes: &[FileChange],
    base: &str,
    head: Option<&str>,
) -> Impact {
    let by_id: HashMap<&str, &Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let lift = |id: &str| -> Option<String> {
        let mut cur = *by_id.get(id)?;
        for _ in 0..=graph.symbols.len() {
            match cur.container.as_deref().and_then(|c| by_id.get(c)) {
                Some(p) => cur = p,
                None => return Some(cur.id.clone()),
            }
        }
        None
    };

    let mut changed_files: Vec<String> = changes.iter().map(|c| c.file.clone()).collect();
    changed_files.sort();
    changed_files.dedup();

    let mut changed: BTreeSet<String> = BTreeSet::new();
    for ch in changes {
        for s in graph.symbols_in_file(&ch.file) {
            let st = s.range.start_line;
            let en = (s.range.end_line + u32::from(s.range.end_col > 0)).max(st + 1);
            if ch.lines.is_empty() || ch.lines.iter().any(|&(a, b)| st < b && a < en) {
                changed.extend(lift(&s.id));
            }
        }
    }

    // callers[x] = classes that depend on x
    let mut callers: HashMap<String, BTreeSet<String>> = HashMap::new();
    for e in &graph.edges {
        if let (Some(f), Some(t)) = (lift(&e.from), lift(&e.to))
            && f != t
        {
            callers.entry(t).or_default().insert(f);
        }
    }
    let mut dist: HashMap<String, u32> = changed.iter().map(|c| (c.clone(), 0)).collect();
    let mut queue: VecDeque<(String, u32)> = changed.iter().map(|c| (c.clone(), 0)).collect();
    while let Some((id, d)) = queue.pop_front() {
        for caller in callers.get(&id).into_iter().flatten() {
            if !dist.contains_key(caller) {
                dist.insert(caller.clone(), d + 1);
                queue.push_back((caller.clone(), d + 1));
            }
        }
    }
    let mut impacted_symbols: Vec<ImpactedSymbol> = dist
        .iter()
        .filter(|(id, _)| !changed.contains(*id))
        .map(|(id, &distance)| ImpactedSymbol {
            id: id.clone(),
            distance,
        })
        .collect();
    impacted_symbols.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));

    let cell_of = |id: &str| {
        cls.placements
            .get(id)
            .map(|p| Cell::id_for(p.layer, &p.domain))
    };
    let changed_cells: BTreeSet<String> = changed.iter().filter_map(|s| cell_of(s)).collect();
    let mut cell_dist: BTreeMap<String, u32> = BTreeMap::new();
    for s in &impacted_symbols {
        if let Some(c) = cell_of(&s.id)
            && !changed_cells.contains(&c)
        {
            let e = cell_dist.entry(c).or_insert(s.distance);
            *e = (*e).min(s.distance);
        }
    }
    let mut impacted_cells: Vec<ImpactedCell> = cell_dist
        .into_iter()
        .map(|(id, distance)| ImpactedCell { id, distance })
        .collect();
    impacted_cells.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));

    Impact {
        base: base.to_string(),
        head: head.map(str::to_string),
        changed_files,
        changed_symbols: changed.into_iter().collect(),
        changed_cells: changed_cells.into_iter().collect(),
        impacted_cells,
        impacted_symbols,
        warnings: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Edge, EdgeKind, Language, Range, SymbolKind};
    use crate::model::{Layer, Placement};

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
        let none = [FileChange {
            file: "Mapper.java".into(),
            lines: vec![(30, 31)],
        }];
        assert!(
            compute(&g, &c, &none, "HEAD", None)
                .changed_symbols
                .is_empty()
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
}
