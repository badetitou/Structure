//! Joins the tree-sitter symbol graph with a SCIP index to add precise
//! `Call` / `Uses` / `Implements` edges.

use std::collections::{HashMap, HashSet};

use crate::ir::{Edge, EdgeKind, Graph, Range, Symbol, SymbolKind};
use crate::scip::{self, ScipIndex};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Distinct SCIP symbols mapped to an ir symbol.
    pub mapped_symbols: usize,
    /// Distinct non-local SCIP symbols with a definition but no ir symbol.
    pub unmapped_definitions: usize,
    /// Non-definition occurrences of non-local symbols seen.
    pub references: usize,
    pub call_edges: usize,
    pub uses_edges: usize,
    pub implements_edges: usize,
    /// Final edges whose target is `ext:*`.
    pub ext_edges: usize,
}

fn rank(k: EdgeKind) -> u8 {
    match k {
        EdgeKind::Call => 0,
        EdgeKind::Implements => 1,
        EdgeKind::Extends => 2,
        EdgeKind::Uses => 3,
    }
}

fn is_ctor_name(n: &str) -> bool {
    matches!(n, "<init>" | "<constructor>" | "constructor" | "init")
}

/// Parameters, type parameters and packages/modules carry no graph meaning.
fn is_noise(symbol: &str) -> bool {
    symbol.ends_with(')') || symbol.ends_with(']') || symbol.ends_with('/')
}

fn is_callable(k: SymbolKind) -> bool {
    matches!(
        k,
        SymbolKind::Method | SymbolKind::Constructor | SymbolKind::Function
    )
}

fn contains(r: &Range, p: (u32, u32)) -> bool {
    (r.start_line, r.start_col) <= p && p < (r.end_line, r.end_col)
}

struct FileIndex {
    /// Symbol indices sorted by range start.
    by_start: Vec<usize>,
    /// name_range start line -> symbol indices.
    by_line: HashMap<u32, Vec<usize>>,
}

fn edge_order(a: &Edge, b: &Edge) -> std::cmp::Ordering {
    (&a.from, &a.to, rank(a.kind)).cmp(&(&b.from, &b.to, rank(b.kind)))
}

pub fn link(graph: Graph, index: &ScipIndex) -> (Graph, LinkStats) {
    let mut graph = graph;
    let mut stats = LinkStats::default();

    let mut files: HashMap<&str, FileIndex> = HashMap::new();
    for (i, s) in graph.symbols.iter().enumerate() {
        let f = files.entry(s.file.as_str()).or_insert_with(|| FileIndex {
            by_start: vec![],
            by_line: HashMap::new(),
        });
        f.by_start.push(i);
        f.by_line
            .entry(s.name_range.start_line)
            .or_default()
            .push(i);
    }
    for f in files.values_mut() {
        f.by_start.sort_by_key(|&i| {
            let r = &graph.symbols[i].range;
            (
                r.start_line,
                r.start_col,
                std::cmp::Reverse((r.end_line, r.end_col)),
            )
        });
    }

    // 1. SCIP symbol -> ir symbol index.
    let mut map: HashMap<&str, usize> = HashMap::new();
    let mut defined: HashSet<&str> = HashSet::new();
    for doc in &index.documents {
        let fi = files.get(doc.relative_path.as_str());
        for o in &doc.occurrences {
            if !o.is_definition || scip::is_local(&o.symbol) || is_noise(&o.symbol) {
                continue;
            }
            defined.insert(o.symbol.as_str());
            if map.contains_key(o.symbol.as_str()) {
                continue;
            }
            let (Some(fi), Some(name)) = (fi, scip::simple_name(&o.symbol)) else {
                continue;
            };
            let Some(cands) = fi.by_line.get(&o.range.start_line) else {
                continue;
            };
            let best = cands
                .iter()
                .copied()
                .filter(|&i| {
                    let s = &graph.symbols[i];
                    s.name == name || (s.kind == SymbolKind::Constructor && is_ctor_name(&name))
                })
                .min_by_key(|&i| {
                    graph.symbols[i]
                        .name_range
                        .start_col
                        .abs_diff(o.range.start_col)
                });
            if let Some(i) = best {
                map.insert(o.symbol.as_str(), i);
            }
        }
    }
    stats.mapped_symbols = map.len();
    stats.unmapped_definitions = defined.iter().filter(|s| !map.contains_key(*s)).count();

    // 2. References.
    let mut new_edges: Vec<Edge> = Vec::new();
    for doc in &index.documents {
        let Some(fi) = files.get(doc.relative_path.as_str()) else {
            continue;
        };
        for o in &doc.occurrences {
            if o.is_definition || scip::is_local(&o.symbol) || is_noise(&o.symbol) {
                continue;
            }
            stats.references += 1;
            let p = (o.range.start_line, o.range.start_col);
            let end = fi.by_start.partition_point(|&i| {
                let r = &graph.symbols[i].range;
                (r.start_line, r.start_col) <= p
            });
            let Some(src) = fi.by_start[..end]
                .iter()
                .rev()
                .copied()
                .find(|&i| contains(&graph.symbols[i].range, p))
            else {
                continue;
            };
            let src_id = &graph.symbols[src].id;
            let (to, kind) = match map.get(o.symbol.as_str()) {
                Some(&t) => {
                    let ts = &graph.symbols[t];
                    let k = if is_callable(ts.kind) {
                        EdgeKind::Call
                    } else {
                        EdgeKind::Uses
                    };
                    (ts.id.clone(), k)
                }
                None => {
                    let name =
                        scip::owner_type_name(&o.symbol).or_else(|| scip::simple_name(&o.symbol));
                    let Some(name) = name else { continue };
                    let k = if o.symbol.ends_with(").") {
                        EdgeKind::Call
                    } else {
                        EdgeKind::Uses
                    };
                    (format!("ext:{name}"), k)
                }
            };
            if &to == src_id {
                continue;
            }
            new_edges.push(Edge {
                from: src_id.clone(),
                to,
                kind,
            });
        }
    }

    // 3. Implementations.
    let mut impls_of: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut syms: Vec<&String> = index.symbols.keys().collect();
    syms.sort();
    for s in syms {
        let Some(&si) = map.get(s.as_str()) else {
            continue;
        };
        for r in &index.symbols[s].relationships {
            if !r.is_implementation {
                continue;
            }
            let Some(&ti) = map.get(r.symbol.as_str()) else {
                continue;
            };
            if si == ti {
                continue;
            }
            new_edges.push(Edge {
                from: graph.symbols[si].id.clone(),
                to: graph.symbols[ti].id.clone(),
                kind: EdgeKind::Implements,
            });
            if is_callable(graph.symbols[ti].kind) {
                impls_of.entry(ti).or_default().push(si);
            }
        }
    }
    if !impls_of.is_empty() {
        let id_ix: HashMap<&str, usize> = graph
            .symbols
            .iter()
            .enumerate()
            .map(|(i, s): (usize, &Symbol)| (s.id.as_str(), i))
            .collect();
        let mut extra = Vec::new();
        for e in new_edges.iter().filter(|e| e.kind == EdgeKind::Call) {
            let Some(&m) = id_ix.get(e.to.as_str()) else {
                continue;
            };
            let mut seen = HashSet::from([m]);
            let mut stack = vec![m];
            while let Some(c) = stack.pop() {
                for &i in impls_of.get(&c).into_iter().flatten() {
                    if seen.insert(i) {
                        stack.push(i);
                        if graph.symbols[i].id != e.from {
                            extra.push(Edge {
                                from: e.from.clone(),
                                to: graph.symbols[i].id.clone(),
                                kind: EdgeKind::Call,
                            });
                        }
                    }
                }
            }
        }
        new_edges.extend(extra);
    }

    // 4. Merge, dedupe, sort.
    graph.edges.extend(new_edges);
    graph.edges.sort_by(edge_order);
    graph.edges.dedup();
    for e in &graph.edges {
        match e.kind {
            EdgeKind::Call => stats.call_edges += 1,
            EdgeKind::Uses => stats.uses_edges += 1,
            EdgeKind::Implements => stats.implements_edges += 1,
            EdgeKind::Extends => {}
        }
        if e.to.starts_with("ext:") {
            stats.ext_edges += 1;
        }
    }
    (graph, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Language;
    use crate::scip::{ScipDocument, ScipOccurrence, ScipRelationship, ScipSymbol};

    fn r(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
        Range {
            start_line: sl,
            start_col: sc,
            end_line: el,
            end_col: ec,
        }
    }

    fn sym(id: &str, kind: SymbolKind, name: &str, file: &str, range: Range, nr: Range) -> Symbol {
        Symbol {
            id: id.into(),
            kind,
            name: name.into(),
            container: None,
            file: file.into(),
            range,
            name_range: nr,
            annotations: vec![],
            supertypes: vec![],
            package: None,
        }
    }

    fn pfx(s: &str) -> String {
        format!("semanticdb maven m 1 {s}")
    }

    fn occ(s: &str, range: Range, def: bool) -> ScipOccurrence {
        ScipOccurrence {
            symbol: pfx(s),
            range,
            is_definition: def,
            enclosing_range: None,
        }
    }

    fn doc(p: &str, o: Vec<ScipOccurrence>) -> ScipDocument {
        ScipDocument {
            relative_path: p.into(),
            language: "java".into(),
            occurrences: o,
        }
    }

    fn fixture() -> (Graph, ScipIndex) {
        use SymbolKind::*;
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym("p.A", Class, "A", "A.java", r(0, 0, 9, 1), r(0, 6, 0, 7)),
                sym(
                    "p.A#<init>(0)",
                    Constructor,
                    "A",
                    "A.java",
                    r(2, 2, 2, 12),
                    r(2, 2, 2, 3),
                ),
                sym(
                    "p.A#run(0)",
                    Method,
                    "run",
                    "A.java",
                    r(4, 2, 6, 3),
                    r(4, 7, 4, 10),
                ),
                sym("p.B", Class, "B", "B.java", r(0, 0, 5, 1), r(0, 6, 0, 7)),
                sym(
                    "p.B#go(0)",
                    Method,
                    "go",
                    "B.java",
                    r(2, 2, 4, 3),
                    r(2, 7, 2, 9),
                ),
                sym(
                    "p.I",
                    Interface,
                    "I",
                    "I.java",
                    r(0, 0, 3, 1),
                    r(0, 10, 0, 11),
                ),
                sym(
                    "p.I#run(0)",
                    Method,
                    "run",
                    "I.java",
                    r(1, 2, 1, 14),
                    r(1, 7, 1, 10),
                ),
                sym("p.C", Class, "C", "C.java", r(0, 0, 5, 1), r(0, 6, 0, 7)),
                sym(
                    "p.C#go(0)",
                    Method,
                    "go",
                    "C.java",
                    r(2, 2, 4, 3),
                    r(2, 7, 2, 9),
                ),
            ],
            edges: vec![],
        };
        let a = doc(
            "A.java",
            vec![
                occ("p/A#", r(0, 6, 0, 7), true),
                occ("p/A#`<init>`().", r(2, 2, 2, 3), true),
                occ("p/A#run().", r(4, 7, 4, 10), true),
                occ("p/A#run().(x)", r(4, 11, 4, 12), true), // parameter: ignored
                occ("p/I#", r(0, 20, 0, 21), false),
            ],
        );
        let b = doc(
            "B.java",
            vec![
                occ("p/B#", r(0, 6, 0, 7), true),
                occ("p/B#go().", r(2, 7, 2, 9), true),
                occ("p/A#`<init>`().", r(3, 8, 3, 9), false),
                occ("p/A#", r(3, 8, 3, 9), false),
                occ("p/A#run().", r(3, 12, 3, 15), false),
                occ("ext/Foo#bar().", r(3, 16, 3, 19), false),
                occ("ext/Baz#", r(3, 20, 3, 23), false),
                occ("ext/Imported#", r(8, 7, 8, 15), false), // outside every symbol
            ],
        );
        // Import line: no enclosing symbol in this file.
        let imp = doc("Imp.java", vec![occ("p/A#", r(0, 7, 0, 8), false)]);
        let i = doc(
            "I.java",
            vec![
                occ("p/I#", r(0, 10, 0, 11), true),
                occ("p/I#run().", r(1, 7, 1, 10), true),
            ],
        );
        let c = doc(
            "C.java",
            vec![
                occ("p/C#", r(0, 6, 0, 7), true),
                occ("p/C#go().", r(2, 7, 2, 9), true),
                occ("p/I#run().", r(3, 4, 3, 7), false),
            ],
        );
        let mut symbols = HashMap::new();
        let rel = |s: &str| ScipRelationship {
            symbol: pfx(s),
            is_implementation: true,
            is_reference: false,
            is_type_definition: false,
            is_definition: false,
        };
        for (s, to) in [("p/A#", "p/I#"), ("p/A#run().", "p/I#run().")] {
            symbols.insert(
                pfx(s),
                ScipSymbol {
                    symbol: pfx(s),
                    display_name: None,
                    kind: None,
                    relationships: vec![rel(to)],
                },
            );
        }
        (
            g,
            ScipIndex {
                documents: vec![a, b, imp, i, c],
                symbols,
            },
        )
    }

    fn has(g: &Graph, f: &str, t: &str, k: EdgeKind) -> bool {
        g.edges
            .iter()
            .any(|e| e.from == f && e.to == t && e.kind == k)
    }

    #[test]
    fn links() {
        let (g, idx) = fixture();
        let (g, st) = link(g, &idx);
        // Mapped by line+name, incl. ctor (`<init>` vs tree-sitter name "A").
        assert_eq!(st.mapped_symbols, 9);
        assert_eq!(st.unmapped_definitions, 0);
        // Innermost enclosing method; callable target -> Call.
        assert!(has(&g, "p.B#go(0)", "p.A#run(0)", EdgeKind::Call));
        assert!(has(&g, "p.B#go(0)", "p.A#<init>(0)", EdgeKind::Call));
        assert!(has(&g, "p.B#go(0)", "p.A", EdgeKind::Uses));
        // External method -> ext:Owner Call; external type -> Uses.
        assert!(has(&g, "p.B#go(0)", "ext:Foo", EdgeKind::Call));
        assert!(has(&g, "p.B#go(0)", "ext:Baz", EdgeKind::Uses));
        // Import-line reference skipped (only B's 5 and A's 1 refs yield edges).
        assert!(g.edges.iter().all(|e| e.from != "Imp.java"));
        assert_eq!(st.references, 8); // Imp.java has no symbols: not counted
        assert!(!has(&g, "p.B", "ext:Imported", EdgeKind::Uses));
        assert!(g.edges.iter().all(|e| e.to != "ext:Imported"));
        // Reference outside members is attributed to the class.
        assert!(has(&g, "p.A", "p.I", EdgeKind::Uses));
        assert!(has(&g, "p.A", "p.I", EdgeKind::Implements));
        assert!(has(&g, "p.A#run(0)", "p.I#run(0)", EdgeKind::Implements));
        // Interface propagation.
        assert!(has(&g, "p.C#go(0)", "p.I#run(0)", EdgeKind::Call));
        assert!(has(&g, "p.C#go(0)", "p.A#run(0)", EdgeKind::Call));
        assert_eq!(st.ext_edges, 2);
        assert_eq!(st.implements_edges, 2);
    }

    #[test]
    fn deterministic() {
        let (g, idx) = fixture();
        let (a, sa) = link(g.clone(), &idx);
        let (b, sb) = link(g, &idx);
        assert_eq!(a, b);
        assert_eq!(sa, sb);
        let mut sorted = a.edges.clone();
        sorted.sort_by(edge_order);
        assert_eq!(sorted, a.edges);
    }
}
