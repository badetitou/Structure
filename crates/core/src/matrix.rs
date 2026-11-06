//! Quotient graph: symbol-level graph + classification -> layer x domain matrix.

use crate::ir::{EdgeKind, Graph, Symbol};
use crate::model::{Cell, CellEdge, CellSymbol, Classification, Layer, Matrix, Stats};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Cell id of any symbol (lifted to its top-level symbol first).
pub fn cell_of(graph: &Graph, cls: &Classification, symbol_id: &str) -> Option<String> {
    let top = graph.top_level_of(symbol_id)?;
    let p = cls.placements.get(&top.id)?;
    Some(Cell::id_for(p.layer, &p.domain))
}

/// Symbol edges lifted to top-level ids, deduped with counts, self-loops and
/// edges with an unresolved endpoint removed. Sorted by (from, to, kind).
pub fn class_edges(graph: &Graph) -> Vec<(String, String, EdgeKind, u32)> {
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
    let kind_rank = |k: EdgeKind| match k {
        EdgeKind::Call => 0u8,
        EdgeKind::Implements => 1,
        EdgeKind::Extends => 2,
        EdgeKind::Uses => 3,
    };
    let mut counts: BTreeMap<(String, String, u8), (EdgeKind, u32)> = BTreeMap::new();
    for e in &graph.edges {
        let (Some(f), Some(t)) = (lift(&e.from), lift(&e.to)) else {
            continue;
        };
        if f == t {
            continue;
        }
        counts
            .entry((f, t, kind_rank(e.kind)))
            .or_insert((e.kind, 0))
            .1 += 1;
    }
    counts
        .into_iter()
        .map(|((f, t, _), (k, n))| (f, t, k, n))
        .collect()
}

pub fn build(graph: &Graph, cls: &Classification, project: &str, elapsed_ms: u64) -> Matrix {
    let mut grouped: BTreeMap<(Layer, String), Vec<CellSymbol>> = BTreeMap::new();
    for s in graph.symbols.iter().filter(|s| s.container.is_none()) {
        if let Some(p) = cls.placements.get(&s.id) {
            grouped
                .entry((p.layer, p.domain.clone()))
                .or_default()
                .push(CellSymbol {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    file: s.file.clone(),
                });
        }
    }
    let layers: Vec<Layer> = grouped
        .keys()
        .map(|(l, _)| *l)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let domains: Vec<String> = grouped
        .keys()
        .map(|(_, d)| d.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let cells: Vec<Cell> = grouped
        .into_iter()
        .map(|((layer, domain), mut symbols)| {
            symbols.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
            Cell {
                id: Cell::id_for(layer, &domain),
                layer,
                domain,
                size: symbols.len(),
                symbols,
            }
        })
        .collect();

    let cell_of_top = |id: &str| -> Option<String> {
        let p = cls.placements.get(id)?;
        Some(Cell::id_for(p.layer, &p.domain))
    };
    let mut weights: BTreeMap<(String, String), u32> = BTreeMap::new();
    for (f, t, kind, n) in class_edges(graph) {
        if !matches!(kind, EdgeKind::Call | EdgeKind::Uses) {
            continue;
        }
        let (Some(cf), Some(ct)) = (cell_of_top(&f), cell_of_top(&t)) else {
            continue;
        };
        if cf != ct {
            *weights.entry((cf, ct)).or_insert(0) += n;
        }
    }
    let edges = weights
        .into_iter()
        .map(|((from, to), weight)| CellEdge { from, to, weight })
        .collect();

    let files: BTreeSet<&str> = graph.symbols.iter().map(|s| s.file.as_str()).collect();
    Matrix {
        project: project.to_string(),
        language: graph.language,
        layers,
        domains,
        cells,
        edges,
        warnings: vec![],
        stats: Stats {
            files: files.len(),
            symbols: graph.symbols.len(),
            edges: graph.edges.len(),
            elapsed_ms,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Edge, Language, Range, SymbolKind};
    use crate::model::Placement;

    fn sym(id: &str, name: &str, kind: SymbolKind, container: Option<&str>, file: &str) -> Symbol {
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        Symbol {
            id: id.into(),
            kind,
            name: name.into(),
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
        use SymbolKind::*;
        let symbols = vec![
            sym("Ctl", "Ctl", Class, None, "Ctl.java"),
            sym("Ctl#a", "a", Method, Some("Ctl"), "Ctl.java"),
            sym("Ctl2", "Ctl2", Class, None, "Ctl2.java"),
            sym("Svc", "Svc", Class, None, "Svc.java"),
            sym("Svc#m", "m", Method, Some("Svc"), "Svc.java"),
            sym("Svc#n", "n", Method, Some("Svc"), "Svc.java"),
            sym("Aaa", "Aaa", Class, None, "Aaa.java"),
            sym("Repo", "Repo", Class, None, "Repo.java"),
            sym("Repo#q", "q", Method, Some("Repo"), "Repo.java"),
            sym("Repo2", "Repo2", Class, None, "Repo2.java"),
            sym("Orphan", "Orphan", Class, None, "Orphan.java"),
        ];
        let edges = vec![
            edge("Ctl#a", "Svc#m", EdgeKind::Call),
            edge("Ctl#a", "Svc#n", EdgeKind::Call),
            edge("Ctl2", "Svc", EdgeKind::Uses),
            edge("Svc#m", "Repo#q", EdgeKind::Call),
            edge("Svc#m", "Aaa", EdgeKind::Call), // intra-cell
            edge("Svc#m", "ext:HttpClient", EdgeKind::Call),
            edge("Repo", "Svc", EdgeKind::Implements),
            edge("Repo#q", "Svc#m", EdgeKind::Call), // back edge
            edge("Svc#m", "Svc#n", EdgeKind::Call),  // self loop
        ];
        let mut cls = Classification::default();
        for (id, layer, dom) in [
            ("Ctl", Layer::Entry, "account"),
            ("Ctl2", Layer::Entry, "account"),
            ("Svc", Layer::Business, "account"),
            ("Aaa", Layer::Business, "account"),
            ("Repo", Layer::Persistence, "account"),
            ("Repo2", Layer::Persistence, "billing"),
        ] {
            cls.placements.insert(
                id.into(),
                Placement {
                    layer,
                    domain: dom.into(),
                },
            );
        }
        (
            Graph {
                root: ".".into(),
                language: Language::Java,
                symbols,
                edges,
            },
            cls,
        )
    }

    #[test]
    fn builds_cells_and_edges() {
        let (g, cls) = fixture();
        let m = build(&g, &cls, "p", 7);
        assert_eq!(
            m.layers,
            vec![Layer::Entry, Layer::Business, Layer::Persistence]
        );
        assert_eq!(m.domains, vec!["account", "billing"]);
        let ids: Vec<_> = m.cells.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "entry:account",
                "business:account",
                "persistence:account",
                "persistence:billing"
            ]
        );
        let biz = &m.cells[1];
        assert_eq!(biz.size, 2);
        assert_eq!(biz.symbols[0].name, "Aaa");
        assert_eq!(biz.symbols[1].file, "Svc.java");
        let e: Vec<_> = m
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.weight))
            .collect();
        assert_eq!(
            e,
            [
                ("business:account", "persistence:account", 1),
                ("entry:account", "business:account", 3),
                ("persistence:account", "business:account", 1),
            ]
        );
        assert_eq!(m.stats.files, 7);
        assert_eq!(m.stats.symbols, 11);
        assert_eq!(m.stats.edges, 9);
        assert_eq!(m.stats.elapsed_ms, 7);
        assert_eq!(m.project, "p");
    }

    #[test]
    fn cell_of_lifts_methods() {
        let (g, cls) = fixture();
        assert_eq!(
            cell_of(&g, &cls, "Svc#m").as_deref(),
            Some("business:account")
        );
        assert_eq!(cell_of(&g, &cls, "Orphan"), None);
        assert_eq!(cell_of(&g, &cls, "ext:x"), None);
    }

    #[test]
    fn class_edges_dedupe_and_filter() {
        let (g, _) = fixture();
        let ce = class_edges(&g);
        assert!(ce.contains(&("Ctl".into(), "Svc".into(), EdgeKind::Call, 2)));
        assert!(
            ce.iter()
                .all(|(f, t, _, _)| f != t && !t.starts_with("ext:"))
        );
        // Ctl->Svc, Ctl2->Svc, Svc->Repo, Svc->Aaa, Repo->Svc (impl), Repo->Svc (call)
        assert_eq!(ce.len(), 6);
    }

    #[test]
    fn json_roundtrip() {
        let (g, cls) = fixture();
        let m = build(&g, &cls, "p", 1);
        let s = serde_json::to_string(&m).unwrap();
        let back: Matrix = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }
}
