//! Several projects' graphs and classifications merged into one (pure): the
//! symbol ids of project `p` become `p::<id>` (external `ext:` types are
//! shared), its files `p/<file>` and its domains `p/<domain>`. Used by the
//! multi-root and multi-language pipeline of `structure-core` and by the
//! multi-language pull-request analysis of `structure-wasm`.

use std::collections::BTreeMap;

use crate::layers::Layers;
use structure_ir::ir::Graph;
use structure_ir::model::{Classification, DomainNode, LayerDef, Placement};

/// `<project>::<id>`; external types (`ext:`) are shared, never prefixed.
pub fn prefix_id(project: &str, id: &str) -> String {
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

/// Each root's declared layers merged by id. The first (primary) root keeps
/// its layers, labels and orders; a layer only declared by later roots is
/// appended after them, in root order then in that root's order, so the
/// primary root's rows never move. `unknown` stays implicit and last.
pub fn merge_layers<'a>(per_root: impl IntoIterator<Item = &'a Layers>) -> Layers {
    let mut roots = per_root.into_iter();
    let mut defs: Vec<LayerDef> = roots.next().map_or_else(Vec::new, |l| l.defs().to_vec());
    defs.retain(|d| !d.id.is_unknown());
    for layers in roots {
        for d in layers.defs().iter().filter(|d| !d.id.is_unknown()) {
            if defs.iter().any(|l| l.id == d.id) {
                continue;
            }
            let order = defs
                .iter()
                .map(|l| l.order.saturating_add(1))
                .max()
                .unwrap_or(0);
            defs.push(LayerDef { order, ..d.clone() });
        }
    }
    Layers::new(defs)
}

/// Per-root classifications merged into one: symbol ids `<project>::<id>`,
/// domains `<project>/<domain>`, and one domain-tree node per project.
#[derive(Default)]
pub struct MergedClassification {
    placements: BTreeMap<String, Placement>,
    tree: Vec<DomainNode>,
    any_tree: bool,
}

impl MergedClassification {
    /// Adds the classification of project `name`; `sizes` (top-level symbols
    /// placed per domain) make its tree leaves when it has no domain tree.
    pub fn add(&mut self, name: &str, cls: Classification, sizes: BTreeMap<String, usize>) {
        for (id, p) in cls.placements {
            self.placements.insert(
                prefix_id(name, &id),
                Placement {
                    layer: p.layer,
                    domain: format!("{name}/{}", p.domain),
                },
            );
        }
        let children = match &cls.domain_tree {
            Some(t) => {
                self.any_tree = true;
                qualify_tree(name, t)
            }
            // Domains are name roots: one leaf per domain.
            None => sizes
                .into_iter()
                .map(|(d, size)| DomainNode {
                    domain: Some(format!("{name}/{d}")),
                    label: d,
                    size,
                    children: vec![],
                })
                .collect(),
        };
        self.tree.push(DomainNode {
            label: name.to_string(),
            domain: None,
            size: children.iter().map(|c| c.size).sum(),
            children,
        });
    }

    pub fn finish(self) -> Classification {
        Classification {
            placements: self.placements,
            domain_tree: self.any_tree.then_some(self.tree),
        }
    }
}

/// Appends the symbols and the non-remote edges of project `name`'s graph
/// `part` to `graph`, ids, containers and files prefixed. Remote edges are
/// left out: they are linked again on the merged graph.
pub fn append_graph(graph: &mut Graph, name: &str, part: Graph) {
    for mut s in part.symbols {
        s.id = prefix_id(name, &s.id);
        s.container = s.container.map(|c| prefix_id(name, &c));
        s.file = format!("{name}/{}", s.file);
        graph.symbols.push(s);
    }
    for mut e in part.edges.into_iter().filter(|e| !e.kind.is_remote()) {
        e.from = prefix_id(name, &e.from);
        e.to = prefix_id(name, &e.to);
        graph.edges.push(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(defs: &[(&str, &str, u32)]) -> Layers {
        Layers::new(defs.iter().map(|&(id, label, order)| LayerDef {
            id: id.into(),
            label: label.into(),
            order,
        }))
    }

    fn rows(l: &Layers) -> Vec<(String, String, u32)> {
        l.defs()
            .iter()
            .map(|d| (d.id.to_string(), d.label.clone(), d.order))
            .collect()
    }

    #[test]
    fn first_root_layers_win_and_later_ones_are_appended() {
        let first = layers(&[("entry", "Entry", 3), ("model", "Model", 6)]);
        let second = layers(&[("view", "Views", 0), ("entry", "Other", 1)]);
        let third = layers(&[("jobs", "Jobs", 0), ("view", "Screens", 9)]);
        let merged = merge_layers([&first, &second, &third]);
        let row = |id: &str, label: &str, order| (id.to_string(), label.to_string(), order);
        assert_eq!(
            rows(&merged),
            [
                row("entry", "Entry", 3),
                row("model", "Model", 6),
                row("view", "Views", 7),
                row("jobs", "Jobs", 8),
                row("unknown", "unknown", 9),
            ]
        );
    }

    #[test]
    fn identical_layers_merge_unchanged() {
        let a = layers(&[("ui", "ui", 0), ("model", "model", 6)]);
        assert_eq!(merge_layers([&a, &a.clone()]), a);
    }

    #[test]
    fn later_root_unknown_does_not_leak_in() {
        let first = layers(&[("ui", "ui", 0), ("model", "model", 6)]);
        // A later root declaring `unknown` (custom label, early order) next to
        // a layer already known: nothing of it reaches the merged layers.
        let later = layers(&[("unknown", "Misc", 0), ("ui", "Screens", 3)]);
        assert_eq!(merge_layers([&first, &later]), first);
    }

    #[test]
    fn single_root_layers_are_kept() {
        let only = layers(&[("view", "Views", 2), ("entry", "Entry", 0)]);
        assert_eq!(merge_layers([&only]), only);
    }

    #[test]
    fn no_root_gives_only_unknown() {
        assert_eq!(merge_layers([]), Layers::new([]));
    }

    #[test]
    fn append_graph_prefixes_all_but_external_and_remote() {
        use structure_ir::ir::{Edge, EdgeKind, Language, Range, Symbol, SymbolKind};
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        let sym = |id: &str, container: Option<&str>| Symbol {
            id: id.into(),
            kind: SymbolKind::Class,
            name: id.into(),
            container: container.map(str::to_string),
            file: "src/A.java".into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            package: None,
            ports: vec![],
        };
        let part = Graph {
            root: "r".into(),
            language: Language::Java,
            symbols: vec![sym("p.A", None), sym("p.A.run", Some("p.A"))],
            edges: vec![
                Edge {
                    from: "p.A".into(),
                    to: "ext:String".into(),
                    kind: EdgeKind::Uses,
                },
                Edge {
                    from: "p.A".into(),
                    to: "q.B".into(),
                    kind: EdgeKind::Remote {
                        protocol: "http".into(),
                    },
                },
            ],
        };
        let mut g = Graph {
            root: "r".into(),
            language: Language::Java,
            symbols: vec![],
            edges: vec![],
        };
        append_graph(&mut g, "app", part);
        assert_eq!(g.symbols[0].id, "app::p.A");
        assert_eq!(g.symbols[1].container.as_deref(), Some("app::p.A"));
        assert_eq!(g.symbols[0].file, "app/src/A.java");
        assert_eq!(g.edges.len(), 1);
        assert_eq!(
            (g.edges[0].from.as_str(), g.edges[0].to.as_str()),
            ("app::p.A", "ext:String")
        );
    }
}
