//! Classification and matrix recomputed from an analysis' graph with the
//! rules as they are now on disk: no indexing, no parsing, no linking.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::multi::{MergedClassification, merge_layers};
use super::{Analysis, rules_for};
use crate::classify::classify;
use crate::ir::{Edge, Graph, Language};
use crate::layers::Layers;
use crate::matrix;
use crate::model::{Classification, Matrix};

/// `a` reclassified with the rules of its roots as they are now (language
/// defaults plus each root's `.structure/rules.toml`, merged across roots as
/// in [`super::analyze_many`]). The graph and the impact index are shared with
/// `a`; the matrix keeps `a`'s project, projects, warnings and stats, with
/// generation 0 (the caller sets it). An invalid rules file is an error
/// naming it; `a` is untouched.
pub fn reclassify(a: &Analysis) -> anyhow::Result<Analysis> {
    let (classification, layers) = match a.roots.as_slice() {
        [] => bail!("no project root in the analysis"),
        [root] => classify_root(&a.graph, root, a.graph.language)?,
        roots => {
            let projects = &a.matrix.projects;
            if projects.len() != roots.len() {
                bail!(
                    "internal error: {} roots but {} projects",
                    roots.len(),
                    projects.len()
                );
            }
            let mut merged = MergedClassification::default();
            let mut per_root = Vec::with_capacity(roots.len());
            for (root, p) in roots.iter().zip(projects) {
                // One root at a time: its part of the graph is copied while
                // it is classified, never the whole graph.
                let part = root_graph(&a.graph, &p.name, root, p.language);
                let (cls, layers) = classify_root(&part, root, p.language)
                    .with_context(|| format!("project {}", p.name))?;
                let sizes = domain_sizes(&part, &cls);
                merged.add(&p.name, cls, sizes);
                per_root.push(layers);
            }
            (merged.finish(), merge_layers(&per_root))
        }
    };
    let old = &a.matrix;
    let mut m: Matrix = matrix::build(
        &a.graph,
        &classification,
        &layers,
        &old.project,
        old.stats.elapsed_ms,
    );
    m.projects = old.projects.clone();
    m.warnings = old.warnings.clone();
    Ok(Analysis {
        graph: a.graph.clone(),
        roots: a.roots.clone(),
        classification,
        matrix: m,
        link_stats: a.link_stats,
        remote_links: a.remote_links.clone(),
        metrics: a.metrics.clone(),
        warnings: a.warnings.clone(),
        layers,
        impact_index: a.impact_index.clone(),
    })
}

fn classify_root(
    graph: &Graph,
    root: &Path,
    lang: Language,
) -> anyhow::Result<(Classification, Layers)> {
    let rules = rules_for(root, lang)?;
    let sep = crate::lang::pure(lang).module_separator();
    Ok((classify(graph, &rules, sep), rules.layers()))
}

/// The part of a merged graph that came from project `name`, with its ids and
/// files as the single-root analysis had them. Http and Rpc edges are left
/// out: they are linked on the merged graph and classification ignores them.
fn root_graph(graph: &Graph, name: &str, root: &Path, language: Language) -> Graph {
    let pre = format!("{name}::");
    let dir = format!("{name}/");
    let local = |id: &str| -> Option<String> {
        if id.starts_with("ext:") {
            Some(id.to_string())
        } else {
            id.strip_prefix(&pre).map(str::to_string)
        }
    };
    let symbols = graph
        .symbols
        .iter()
        .filter_map(|s| {
            let id = s.id.strip_prefix(&pre)?;
            let mut s = s.clone();
            s.id = id.to_string();
            s.container = s.container.as_deref().and_then(local);
            if let Some(f) = s.file.strip_prefix(&dir) {
                s.file = f.to_string();
            }
            Some(s)
        })
        .collect();
    let edges = graph
        .edges
        .iter()
        .filter(|e| !e.kind.is_remote())
        .filter_map(|e| {
            Some(Edge {
                from: e.from.strip_prefix(&pre)?.to_string(),
                to: local(&e.to)?,
                kind: e.kind.clone(),
            })
        })
        .collect();
    Graph {
        root: root.display().to_string(),
        language,
        symbols,
        edges,
    }
}

/// Top-level symbols placed per domain (the matrix's cell sizes summed over
/// layers).
fn domain_sizes(graph: &Graph, cls: &Classification) -> BTreeMap<String, usize> {
    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    for s in graph.symbols.iter().filter(|s| s.container.is_none()) {
        if let Some(p) = cls.placements.get(&s.id) {
            *sizes.entry(p.domain.clone()).or_default() += 1;
        }
    }
    sizes
}

/// Changes whenever the rules [`reclassify`] reads for `roots` may have
/// changed: each analysed root (after [`super::expand_roots`]) with the size
/// and modification time of its `.structure/rules.toml`, or `-` when it has
/// none. A root holding several languages counts once. Opaque; compare stamps
/// for equality only.
pub fn rules_stamp(roots: &[PathBuf]) -> anyhow::Result<String> {
    Ok(rules_stamp_of(&super::expand_roots(roots)?))
}

/// [`rules_stamp`] of roots already expanded, e.g. an [`Analysis::roots`]:
/// no detection, no directory listing, one `stat` per root.
pub fn rules_stamp_of(roots: &[PathBuf]) -> String {
    // One analysis unit per language: a root's units are adjacent.
    let mut unique: Vec<&PathBuf> = roots.iter().collect();
    unique.dedup();
    let lines: Vec<String> = unique
        .into_iter()
        .map(|r| {
            let file = r.join(super::PROJECT_RULES);
            let state = match std::fs::metadata(&file) {
                Ok(m) => {
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_nanos());
                    format!("{}\t{mtime}", m.len())
                }
                Err(_) => "-".to_string(),
            };
            format!("{}\t{state}", r.display())
        })
        .collect();
    lines.join("\n")
}
