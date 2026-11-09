//! Several roots analysed into one graph and matrix.
//!
//! Each root runs the single-root pipeline (own language, SCIP cache, rules);
//! with at least two roots the results are merged: symbol ids become
//! `<project>::<id>`, files `<project>/<file>` and domains `<project>/<domain>`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::bail;

use super::{Analysis, Options, analyze_in, project_name};
use crate::ir::{EdgeKind, Graph};
use crate::matrix;
use crate::model::{Classification, DomainNode, Matrix, Placement, Progress, Project};

/// Root dir names, suffixed `-2`, `-3`... when they clash.
pub(super) fn unique_names(roots: &[PathBuf]) -> Vec<String> {
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    roots
        .iter()
        .map(|r| {
            let base = project_name(r);
            let mut name = base.clone();
            let mut n = 1;
            while !used.insert(name.clone()) {
                n += 1;
                name = format!("{base}-{n}");
            }
            name
        })
        .collect()
}

/// One root behaves exactly as [`super::analyze`].
pub fn analyze_many(
    roots: &[PathBuf],
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Analysis> {
    match roots {
        [] => bail!("no project root given"),
        [one] => super::analyze(one, opts, progress),
        _ => {
            let names = unique_names(roots);
            let total = roots.len();
            let mut parts = Vec::with_capacity(total);
            for (i, root) in roots.iter().enumerate() {
                let name = &names[i];
                let scaled = |p: Progress| {
                    let pct = ((i * 100 + usize::from(p.pct)) / total) as u8;
                    progress(Progress {
                        step: format!("{name}: {}", p.step),
                        pct,
                        detail: p.detail,
                    })
                };
                parts.push(analyze_in(root, opts, &scaled, false)?);
            }
            Ok(merge(roots, names, parts))
        }
    }
}

fn prefix_id(project: &str, id: &str) -> String {
    if id.starts_with("ext:") {
        id.to_string()
    } else {
        format!("{project}::{id}")
    }
}

fn qualify_tree(project: &str, nodes: &[DomainNode]) -> Vec<DomainNode> {
    nodes
        .iter()
        .map(|n| DomainNode {
            label: n.label.clone(),
            domain: n.domain.as_ref().map(|d| format!("{project}/{d}")),
            size: n.size,
            children: qualify_tree(project, &n.children),
        })
        .collect()
}

fn merge(roots: &[PathBuf], names: Vec<String>, parts: Vec<Analysis>) -> Analysis {
    let mut graph = Graph {
        root: parts[0].graph.root.clone(),
        language: parts[0].graph.language,
        symbols: vec![],
        edges: vec![],
    };
    let mut placements: BTreeMap<String, Placement> = BTreeMap::new();
    let mut tree: Vec<DomainNode> = vec![];
    let mut any_tree = false;
    let mut projects: Vec<Project> = vec![];
    let mut warnings: Vec<String> = vec![];
    let mut metrics = crate::metrics::Metrics::default();
    // The first root's link stats only (not summed, not a later root's).
    let link_stats = parts[0].link_stats;
    let mut elapsed = 0;

    for ((root, name), a) in roots.iter().zip(&names).zip(parts) {
        projects.push(Project {
            name: name.clone(),
            root: root.display().to_string(),
            language: a.graph.language,
        });
        for mut s in a.graph.symbols {
            s.id = prefix_id(name, &s.id);
            s.container = s.container.map(|c| prefix_id(name, &c));
            s.file = format!("{name}/{}", s.file);
            graph.symbols.push(s);
        }
        // Rpc edges are linked again on the merged graph below.
        for mut e in a
            .graph
            .edges
            .into_iter()
            .filter(|e| e.kind != EdgeKind::Rpc)
        {
            e.from = prefix_id(name, &e.from);
            e.to = prefix_id(name, &e.to);
            graph.edges.push(e);
        }
        for (id, p) in a.classification.placements {
            placements.insert(
                prefix_id(name, &id),
                Placement {
                    layer: p.layer,
                    domain: format!("{name}/{}", p.domain),
                },
            );
        }
        let children = match &a.classification.domain_tree {
            Some(t) => {
                any_tree = true;
                qualify_tree(name, t)
            }
            // Domains are name roots: one leaf per domain.
            None => {
                let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
                for c in &a.matrix.cells {
                    *sizes.entry(c.domain.as_str()).or_default() += c.size;
                }
                sizes
                    .into_iter()
                    .map(|(d, size)| DomainNode {
                        label: d.to_string(),
                        domain: Some(format!("{name}/{d}")),
                        size,
                        children: vec![],
                    })
                    .collect()
            }
        };
        tree.push(DomainNode {
            label: name.clone(),
            domain: None,
            size: children.iter().map(|c| c.size).sum(),
            children,
        });
        warnings.extend(a.warnings.into_iter().map(|w| format!("{name}: {w}")));
        elapsed += a.matrix.stats.elapsed_ms;
        metrics.phases.extend(a.metrics.phases);
        metrics.scip_documents += a.metrics.scip_documents;
        metrics.scip_occurrences += a.metrics.scip_occurrences;
    }
    let (http_edges, http_stats) = crate::http_link::link_http(&graph);
    graph.edges.extend(http_edges);
    let (rpc_edges, rpc_stats) = crate::rpc_link::link_rpc(&graph);
    graph.edges.extend(rpc_edges);
    metrics.record_edges(&graph);

    let classification = Classification {
        placements,
        domain_tree: any_tree.then_some(tree),
    };
    let mut matrix: Matrix = matrix::build(&graph, &classification, &names[0], elapsed);
    matrix.projects = projects;
    matrix.warnings = warnings.clone();
    Analysis {
        graph,
        classification,
        matrix,
        link_stats,
        http_stats: Some(http_stats),
        rpc_stats: (rpc_stats.calls + rpc_stats.maybe_rpc > 0).then_some(rpc_stats),
        metrics,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_deduplicated() {
        let roots: Vec<PathBuf> = ["/a/app", "/b/app", "/c/web", "/d/app"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(unique_names(&roots), ["app", "app-2", "web", "app-3"]);
    }

    #[test]
    fn literal_names_cannot_collide_with_suffixed_ones() {
        let roots: Vec<PathBuf> = ["/a/app", "/b/app", "/c/app-2"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let names = unique_names(&roots);
        let set: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(set.len(), 3, "{names:?}");
    }
}
