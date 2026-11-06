//! Quotient graph: symbol-level graph + classification -> layer x domain matrix.

use crate::ir::{EdgeKind, Graph, Symbol};
use crate::model::{
    Cell, CellSymbol, CellSymbols, Classification, EdgeGroup, Layer, Matrix, Stats,
};
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

/// Top-level symbols of the given cells, sorted by (name, id) within a cell and
/// concatenated in the order of `cell_ids`; `limit` symbols from `offset`.
pub fn cell_symbols(
    graph: &Graph,
    cls: &Classification,
    cell_ids: &[String],
    offset: usize,
    limit: usize,
) -> CellSymbols {
    let mut per_cell: HashMap<&str, Vec<CellSymbol>> =
        cell_ids.iter().map(|c| (c.as_str(), Vec::new())).collect();
    for s in graph.symbols.iter().filter(|s| s.container.is_none()) {
        let Some(p) = cls.placements.get(&s.id) else {
            continue;
        };
        let id = Cell::id_for(p.layer, &p.domain);
        if let Some(v) = per_cell.get_mut(id.as_str()) {
            v.push(CellSymbol {
                id: s.id.clone(),
                name: s.name.clone(),
                file: s.file.clone(),
            });
        }
    }
    let mut total = 0;
    let mut page = Vec::new();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    for id in cell_ids {
        if !done.insert(id.as_str()) {
            continue;
        }
        let Some(mut v) = per_cell.remove(id.as_str()) else {
            continue;
        };
        v.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
        let start = offset.saturating_sub(total).min(v.len());
        let room = limit.saturating_sub(page.len());
        total += v.len();
        page.extend(v.into_iter().skip(start).take(room));
    }
    CellSymbols {
        total,
        offset,
        symbols: page,
    }
}

pub fn build(graph: &Graph, cls: &Classification, project: &str, elapsed_ms: u64) -> Matrix {
    let mut sizes: BTreeMap<(Layer, String), usize> = BTreeMap::new();
    for s in graph.symbols.iter().filter(|s| s.container.is_none()) {
        if let Some(p) = cls.placements.get(&s.id) {
            *sizes.entry((p.layer, p.domain.clone())).or_default() += 1;
        }
    }
    let layers: Vec<Layer> = sizes
        .keys()
        .map(|(l, _)| *l)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let domains: Vec<String> = sizes
        .keys()
        .map(|(_, d)| d.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let cells: Vec<Cell> = sizes
        .into_iter()
        .map(|((layer, domain), size)| Cell {
            id: Cell::id_for(layer, &domain),
            layer,
            domain,
            size,
        })
        .collect();
    let index: HashMap<&str, usize> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();

    let cell_of_top = |id: &str| -> Option<String> {
        let p = cls.placements.get(id)?;
        Some(Cell::id_for(p.layer, &p.domain))
    };
    let mut weights: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    for (f, t, kind, n) in class_edges(graph) {
        if !matches!(kind, EdgeKind::Call | EdgeKind::Uses) {
            continue;
        }
        let (Some(cf), Some(ct)) = (cell_of_top(&f), cell_of_top(&t)) else {
            continue;
        };
        let (Some(&i), Some(&j)) = (index.get(cf.as_str()), index.get(ct.as_str())) else {
            continue;
        };
        if i != j {
            *weights.entry((i, j)).or_insert(0) += n;
        }
    }
    let mut edges: Vec<EdgeGroup> = Vec::new();
    for ((from, to), weight) in weights {
        match edges.last_mut() {
            Some(g) if g.from == from => g.targets.push((to, weight)),
            _ => edges.push(EdgeGroup {
                from,
                targets: vec![(to, weight)],
            }),
        }
    }

    let files: BTreeSet<&str> = graph.symbols.iter().map(|s| s.file.as_str()).collect();
    Matrix {
        project: project.to_string(),
        language: graph.language,
        layers,
        domains,
        cells,
        edges,
        warnings: vec![],
        generation: 0,
        domain_tree: cls.domain_tree.clone(),
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
        let page = cell_symbols(&g, &cls, &["business:account".to_string()], 0, 10);
        assert_eq!(page.total, 2);
        assert_eq!(page.symbols[0].name, "Aaa");
        assert_eq!(page.symbols[1].file, "Svc.java");
        let e: Vec<_> = m
            .edges
            .iter()
            .flat_map(|g| {
                let m = &m;
                g.targets
                    .iter()
                    .map(move |&(t, w)| (m.cells[g.from].id.as_str(), m.cells[t].id.as_str(), w))
            })
            .collect();
        assert_eq!(
            e,
            [
                ("entry:account", "business:account", 3),
                ("business:account", "persistence:account", 1),
                ("persistence:account", "business:account", 1),
            ]
        );
        assert_eq!(m.edges.len(), 3, "one group per source cell");
        assert_eq!(m.stats.files, 7);
        assert_eq!(m.stats.symbols, 11);
        assert_eq!(m.stats.edges, 9);
        assert_eq!(m.stats.elapsed_ms, 7);
        assert_eq!(m.project, "p");
    }

    #[test]
    fn cell_symbols_pages_across_cells() {
        let (g, cls) = fixture();
        let cells: Vec<String> = ["business:account", "entry:account", "business:account"]
            .map(String::from)
            .into();
        let all = cell_symbols(&g, &cls, &cells, 0, 100);
        let m = build(&g, &cls, "p", 0);
        let want: usize = m.cells[..2].iter().map(|c| c.size).sum();
        assert_eq!(all.total, want, "duplicate cell ids count once");
        assert_eq!(all.symbols.len(), want);
        let p = cell_symbols(&g, &cls, &cells, 1, 1);
        assert_eq!((p.total, p.offset), (want, 1));
        assert_eq!(p.symbols.len(), 1);
        assert_eq!(p.symbols[0].id, all.symbols[1].id);
        assert!(cell_symbols(&g, &cls, &cells, 9, 5).symbols.is_empty());
        assert_eq!(
            cell_symbols(&g, &cls, &["nope:x".to_string()], 0, 5).total,
            0
        );
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
