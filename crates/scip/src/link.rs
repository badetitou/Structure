//! Joins the tree-sitter symbol graph with a SCIP index to add precise
//! `Call` / `Uses` / `Implements` edges.

use std::collections::{HashMap, HashSet};

use rayon::prelude::*;

use crate::scip::{self, ScipIndex};
use structure_ir::ir::{Edge, EdgeKind, Graph, Range, Symbol, SymbolKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Distinct SCIP symbols mapped to an ir symbol.
    pub mapped_symbols: usize,
    /// Distinct SCIP symbols defined in graph files that have no ir symbol of
    /// their own but are attributed to an enclosing one (interface members,
    /// nested type literals, implicit constructors...).
    pub lifted_symbols: usize,
    /// Distinct non-local SCIP symbols defined in graph files with neither
    /// their own ir symbol nor a mapped ancestor.
    pub unmapped_definitions: usize,
    /// Non-definition occurrences of non-local symbols seen.
    pub references: usize,
    pub call_edges: usize,
    pub uses_edges: usize,
    pub implements_edges: usize,
    /// Final edges whose target is `ext:*`.
    pub ext_edges: usize,
}

fn is_ctor_name(n: &str) -> bool {
    matches!(n, "<init>" | "<constructor>" | "constructor" | "init")
}

/// Parameters, type parameters, packages/modules and meta descriptors
/// (object-literal keys in scip-typescript) carry no graph meaning.
fn is_noise(symbol: &str) -> bool {
    symbol.ends_with(')') || symbol.ends_with(']') || symbol.ends_with('/') || symbol.ends_with(':')
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

struct FileIndex<'a> {
    /// Symbol indices sorted by range start.
    by_start: Vec<usize>,
    /// name_range start line -> symbol indices.
    by_line: HashMap<u32, Vec<usize>>,
    /// name -> symbol indices, in `by_start` order.
    by_name: HashMap<&'a str, Vec<usize>>,
}

/// Edge target before ids are materialised as strings.
enum Target {
    Sym(usize),
    Ext(String),
}

fn edge_order(a: &Edge, b: &Edge) -> std::cmp::Ordering {
    (&a.from, &a.to, a.kind.rank()).cmp(&(&b.from, &b.to, b.kind.rank()))
}

pub fn link(graph: Graph, index: &ScipIndex) -> (Graph, LinkStats) {
    let mut graph = graph;
    let mut stats = LinkStats::default();

    let mut files: HashMap<&str, FileIndex<'_>> = HashMap::new();
    for (i, s) in graph.symbols.iter().enumerate() {
        let f = files.entry(s.file.as_str()).or_insert_with(|| FileIndex {
            by_start: vec![],
            by_line: HashMap::new(),
            by_name: HashMap::new(),
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
        for &i in &f.by_start {
            f.by_name
                .entry(graph.symbols[i].name.as_str())
                .or_default()
                .push(i);
        }
    }

    // 1. SCIP symbol -> ir symbol index.
    let name_matches = |s: &Symbol, name: &str| {
        s.name == name || (s.kind == SymbolKind::Constructor && is_ctor_name(name))
    };
    // Every symbol the project defines, in any document (tests included).
    let mut project: HashSet<&str> = HashSet::new();
    // First definition of each symbol in a document the frontend parsed.
    let mut defs: Vec<(&FileIndex, &scip::ScipOccurrence)> = Vec::new();
    for doc in &index.documents {
        let fi = files.get(doc.relative_path.as_str());
        for o in &doc.occurrences {
            if !o.is_definition || scip::is_local(&o.symbol) || is_noise(&o.symbol) {
                continue;
            }
            if !project.insert(o.symbol.as_str()) {
                continue;
            }
            if let Some(fi) = fi {
                defs.push((fi, o));
            }
        }
    }
    let defined: Vec<&str> = defs.iter().map(|(_, o)| o.symbol.as_str()).collect();
    // Matching is independent per definition; the ordered collect keeps it
    // deterministic.
    let mapped: Vec<Option<usize>> = defs
        .par_iter()
        .map(|&(fi, o)| {
            let name = scip::simple_name(&o.symbol)?;
            // Same identifier line (columns differ: bytes vs UTF-16)...
            let on_line = fi.by_line.get(&o.range.start_line).and_then(|c| {
                c.iter()
                    .copied()
                    .filter(|&i| name_matches(&graph.symbols[i], &name))
                    .min_by_key(|&i| {
                        graph.symbols[i]
                            .name_range
                            .start_col
                            .abs_diff(o.range.start_col)
                    })
            });
            // ...or a same-named symbol whose extent starts at or before the
            // occurrence and whose identifier comes after it (scip-java puts
            // method definitions on the leading annotation line).
            on_line.or_else(|| {
                let p = (o.range.start_line, o.range.start_col);
                let ok = |&i: &usize| {
                    let s = &graph.symbols[i];
                    name_matches(s, &name)
                        && contains(&s.range, p)
                        && (s.name_range.start_line, s.name_range.start_col) >= p
                };
                if is_ctor_name(&name) {
                    // Constructors match under any ctor spelling. Only
                    // symbols starting at or before `p` can contain it.
                    let end = fi.by_start.partition_point(|&i| {
                        let r = &graph.symbols[i].range;
                        (r.start_line, r.start_col) <= p
                    });
                    fi.by_start[..end].iter().rev().copied().find(ok)
                } else {
                    let c = fi.by_name.get(name.as_str())?;
                    c.iter().rev().copied().find(ok)
                }
            })
        })
        .collect();
    let mut map: HashMap<&str, usize> = HashMap::new();
    for (&sym, m) in defined.iter().zip(&mapped) {
        if let Some(i) = *m {
            map.insert(sym, i);
        }
    }
    stats.mapped_symbols = map.len();
    // Attribute the rest to their nearest mapped ancestor (`User#email.` →
    // `User#`, `C#m().(p)lit:x.` → `C#m().`, implicit `C#<init>().` → `C#`).
    let lifted_of: Vec<Option<usize>> = defined
        .par_iter()
        .map(|&sym| {
            if map.contains_key(sym) {
                return None;
            }
            let mut cur = scip::parent_symbol(sym);
            while let Some(p) = cur {
                if let Some(&i) = map.get(p.as_str()) {
                    return Some(i);
                }
                cur = scip::parent_symbol(&p);
            }
            None
        })
        .collect();
    let mut lifted: HashMap<&str, usize> = HashMap::new();
    for (&sym, l) in defined.iter().zip(&lifted_of) {
        match *l {
            Some(i) => {
                lifted.insert(sym, i);
            }
            None if !map.contains_key(sym) => stats.unmapped_definitions += 1,
            None => {}
        }
    }
    stats.lifted_symbols = lifted.len();
    let resolve = |sym: &str| map.get(sym).or_else(|| lifted.get(sym)).copied();

    // 2. References, one document per task. Edges stay compact (symbol index
    // or external name) and are deduplicated per document before any id
    // string is cloned; the ordered collect keeps the result deterministic.
    type Found = (usize, Target, EdgeKind);
    let per_doc: Vec<(usize, Vec<Found>)> = index
        .documents
        .par_iter()
        .map(|doc| {
            let mut refs = 0usize;
            let mut found: Vec<Found> = Vec::new();
            let Some(fi) = files.get(doc.relative_path.as_str()) else {
                return (refs, found);
            };
            for o in &doc.occurrences {
                if o.is_definition || scip::is_local(&o.symbol) || is_noise(&o.symbol) {
                    continue;
                }
                refs += 1;
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
                let (to, kind) = match resolve(o.symbol.as_str()) {
                    Some(t) => {
                        let k = if is_callable(graph.symbols[t].kind)
                            && map.contains_key(o.symbol.as_str())
                        {
                            EdgeKind::Call
                        } else {
                            EdgeKind::Uses
                        };
                        if &graph.symbols[t].id == src_id {
                            continue;
                        }
                        (Target::Sym(t), k)
                    }
                    // A project symbol the frontend doesn't model (file-level
                    // constant, test class): not an external dependency.
                    None if project.contains(o.symbol.as_str()) => continue,
                    None => {
                        let name = scip::owner_type_name(&o.symbol)
                            .or_else(|| scip::simple_name(&o.symbol));
                        let Some(name) = name else { continue };
                        let k = if o.symbol.ends_with(").") {
                            EdgeKind::Call
                        } else {
                            EdgeKind::Uses
                        };
                        let to = format!("ext:{name}");
                        if &to == src_id {
                            continue;
                        }
                        (Target::Ext(to), k)
                    }
                };
                found.push((src, to, kind));
            }
            fn key(e: &Found) -> (usize, usize, &str, u8) {
                match &e.1 {
                    Target::Sym(i) => (e.0, *i, "", e.2.rank()),
                    Target::Ext(n) => (e.0, usize::MAX, n.as_str(), e.2.rank()),
                }
            }
            found.sort_unstable_by(|a, b| key(a).cmp(&key(b)));
            found.dedup_by(|a, b| key(a) == key(b));
            (refs, found)
        })
        .collect();
    let mut new_edges: Vec<Edge> = Vec::new();
    for (refs, found) in per_doc {
        stats.references += refs;
        for (src, to, kind) in found {
            let to = match to {
                Target::Sym(t) => graph.symbols[t].id.clone(),
                Target::Ext(n) => n,
            };
            new_edges.push(Edge {
                from: graph.symbols[src].id.clone(),
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
    graph.edges.par_sort_unstable_by(edge_order);
    graph.edges.dedup();
    for e in &graph.edges {
        match e.kind {
            EdgeKind::Call => stats.call_edges += 1,
            EdgeKind::Uses => stats.uses_edges += 1,
            EdgeKind::Implements => stats.implements_edges += 1,
            EdgeKind::Extends | EdgeKind::Http | EdgeKind::Rpc => {}
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
    use crate::scip::{ScipDocument, ScipOccurrence, ScipRelationship, ScipSymbol};
    use structure_ir::ir::Language;

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
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
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

    /// The same reference repeated within a document and across documents
    /// yields one edge, identically on every run.
    #[test]
    fn duplicate_references_collapse() {
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym(
                    "p.X",
                    SymbolKind::Class,
                    "X",
                    "X.java",
                    r(0, 0, 9, 1),
                    r(0, 6, 0, 7),
                ),
                sym(
                    "p.Y",
                    SymbolKind::Class,
                    "Y",
                    "Y.java",
                    r(0, 0, 9, 1),
                    r(0, 6, 0, 7),
                ),
            ],
            edges: vec![],
        };
        let refs = |line: u32| {
            vec![
                occ("p/Y#", r(line, 2, line, 3), false),
                occ("p/Y#", r(line + 1, 2, line + 1, 3), false),
                occ("ext/Q#", r(line, 5, line, 6), false),
                occ("ext/Q#", r(line + 1, 5, line + 1, 6), false),
            ]
        };
        let mut first = vec![occ("p/X#", r(0, 6, 0, 7), true)];
        first.extend(refs(2));
        let idx = ScipIndex {
            documents: vec![
                doc("X.java", first),
                doc("X.java", refs(5)),
                doc("Y.java", vec![occ("p/Y#", r(0, 6, 0, 7), true)]),
            ],
            symbols: Default::default(),
        };
        let (a, st) = link(g.clone(), &idx);
        let count = |t: &str| {
            a.edges
                .iter()
                .filter(|e| e.from == "p.X" && e.to == t)
                .count()
        };
        assert_eq!(count("p.Y"), 1);
        assert_eq!(count("ext:Q"), 1);
        assert_eq!(a.edges.len(), 2);
        assert_eq!(st.references, 8);
        assert_eq!((st.uses_edges, st.ext_edges), (2, 1));
        let (b, _) = link(g, &idx);
        assert_eq!(a, b);
    }

    /// scip-java puts method definitions on the annotation line; members
    /// without an ir symbol are lifted to their owner; references to project
    /// symbols the frontend doesn't model are dropped, not turned into `ext:`.
    #[test]
    fn annotation_line_lifting_and_project_refs() {
        let f = "C.java";
        let mut m = sym(
            "C#m(0)",
            SymbolKind::Method,
            "m",
            f,
            r(2, 2, 5, 3),
            r(3, 9, 3, 10),
        );
        m.container = Some("C".into());
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym(
                    "C",
                    SymbolKind::Class,
                    "C",
                    f,
                    r(0, 0, 10, 1),
                    r(0, 13, 0, 14),
                ),
                sym(
                    "D",
                    SymbolKind::Class,
                    "D",
                    "D.java",
                    r(0, 0, 3, 1),
                    r(0, 6, 0, 7),
                ),
                m,
            ],
            edges: vec![],
        };
        let index = ScipIndex {
            documents: vec![
                doc(
                    f,
                    vec![
                        occ("C#", r(0, 13, 0, 14), true),
                        occ("C#m().", r(2, 2, 2, 6), true),
                        occ("C#`<init>`().", r(0, 13, 0, 14), true),
                        occ("C#x.", r(1, 6, 1, 7), true),
                        occ("C#m().", r(8, 4, 8, 5), false),
                    ],
                ),
                doc(
                    "D.java",
                    vec![
                        occ("D#", r(0, 6, 0, 7), true),
                        occ("C#`<init>`().", r(1, 4, 1, 5), false),
                        occ("C#x.", r(2, 4, 2, 5), false),
                        occ("T#", r(2, 8, 2, 9), false),
                    ],
                ),
                doc("T.java", vec![occ("T#", r(0, 6, 0, 7), true)]),
            ],
            symbols: HashMap::new(),
        };
        let (g, st) = link(g, &index);
        assert_eq!(st.mapped_symbols, 3, "{st:?}");
        assert_eq!(st.lifted_symbols, 2, "{st:?}");
        assert_eq!(st.unmapped_definitions, 0, "{st:?}");
        assert!(has(&g, "C", "C#m(0)", EdgeKind::Call));
        assert!(has(&g, "D", "C", EdgeKind::Uses));
        assert!(
            !g.edges.iter().any(|e| e.to.starts_with("ext:")),
            "{:?}",
            g.edges
        );
    }
}
