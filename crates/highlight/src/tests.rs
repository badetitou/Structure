use super::*;
use structure_ir::ir::{Edge, EdgeKind, Language, Range, Symbol, SymbolKind};
use structure_ir::model::{LayerId, Placement};

fn sym(id: &str, container: Option<&str>, file: &str, s: u32, e: u32) -> Symbol {
    named_sym(id, id, container, file, s, e)
}

fn named_sym(id: &str, name: &str, container: Option<&str>, file: &str, s: u32, e: u32) -> Symbol {
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
        name: name.into(),
        container: container.map(Into::into),
        file: file.into(),
        range: r,
        name_range: r,
        annotations: vec![],
        supertypes: vec![],
        ports: vec![],
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

fn graph(symbols: Vec<Symbol>, edges: Vec<Edge>) -> Graph {
    Graph {
        root: ".".into(),
        language: Language::Java,
        symbols,
        edges,
    }
}

/// Mapper <- Service <- Action call chain, Impl/User -> Iface, a lonely class.
fn fixture() -> (Graph, Classification) {
    let g = graph(
        vec![
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
        vec![
            edge("Service#run", "Mapper#map", EdgeKind::Call),
            edge("Action", "Service", EdgeKind::Uses),
            edge("Impl", "Iface", EdgeKind::Implements),
            edge("User", "Iface", EdgeKind::Uses),
            edge("Mapper#map", "Mapper#other", EdgeKind::Call),
            edge("Action", "ext:foo", EdgeKind::Call),
        ],
    );
    let mut c = Classification::default();
    for (id, l, d) in [
        ("Mapper", LayerId::from("persistence"), "a"),
        ("Service", LayerId::from("business"), "a"),
        ("Action", LayerId::from("entry"), "a"),
        ("Iface", LayerId::from("business"), "b"),
        ("Impl", LayerId::from("business"), "b"),
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

/// Indices of `ids` in `g.symbols` (last match, as the index does).
fn idx(g: &Graph, ids: &[&str]) -> BTreeSet<u32> {
    ids.iter()
        .filter_map(|id| g.symbols.iter().rposition(|s| s.id == *id))
        .map(|i| i as u32)
        .collect()
}

fn found(g: &Graph, ids: &[&str]) -> Resolution {
    Resolution::Symbols(idx(g, ids))
}

fn loc(file: &str, lines: Vec<(u32, u32)>) -> Anchor {
    Anchor::Location {
        file: file.into(),
        lines,
    }
}

fn qualified(name: &str, file_hint: Option<&str>, line: Option<u32>) -> Anchor {
    Anchor::Qualified {
        name: name.into(),
        file_hint: file_hint.map(Into::into),
        line,
    }
}

const CALLERS: Propagation = Propagation {
    direction: Direction::Callers,
    max_depth: None,
};

fn by_id(g: &Graph, m: BTreeMap<u32, u32>) -> BTreeMap<String, u32> {
    m.into_iter()
        .map(|(i, d)| (g.symbols[i as usize].id.clone(), d))
        .collect()
}

#[test]
fn resolve_symbol_lifts_to_top_level() {
    let g = graph(
        vec![
            sym("A", None, "A.java", 0, 10),
            sym("A#m", Some("A"), "A.java", 2, 4),
            // Container cycle: never lifted.
            sym("X", Some("Y"), "X.java", 0, 5),
            sym("Y", Some("X"), "X.java", 6, 9),
        ],
        vec![],
    );
    let index = HighlightIndex::new(&g);
    let symbol = |id: &str| Anchor::Symbol { id: id.into() };
    assert_eq!(resolve(&g, &index, &symbol("A")), found(&g, &["A"]));
    assert_eq!(resolve(&g, &index, &symbol("A#m")), found(&g, &["A"]));
    assert_eq!(resolve(&g, &index, &symbol("Nope")), Resolution::Unresolved);
    assert_eq!(resolve(&g, &index, &symbol("X")), Resolution::Unresolved);
}

#[test]
fn resolve_location_lifts_overlaps_to_top_level() {
    let (g, _) = fixture();
    let index = HighlightIndex::new(&g);
    // A method body: its class only.
    assert_eq!(
        resolve(&g, &index, &loc("Mapper.java", vec![(6, 7)])),
        found(&g, &["Mapper"])
    );
    // No lines: the whole file.
    assert_eq!(
        resolve(&g, &index, &loc("Iface.java", vec![])),
        found(&g, &["Iface"])
    );
    // A range past every symbol marks the whole file.
    assert_eq!(
        resolve(&g, &index, &loc("Action.java", vec![(40, 41)])),
        found(&g, &["Action"])
    );
    // An unknown file marks nothing.
    assert_eq!(
        resolve(&g, &index, &loc("Nope.java", vec![(0, 1)])),
        Resolution::Unresolved
    );
}

#[test]
fn resolve_qualified_by_id_then_name() {
    let g = graph(
        vec![
            named_sym("com.a.Foo", "Foo", None, "src/a/Foo.java", 0, 10),
            named_sym(
                "com.a.Foo#Foo",
                "Foo",
                Some("com.a.Foo"),
                "src/a/Foo.java",
                2,
                4,
            ),
            named_sym("com.b.Foo", "Foo", None, "src/b/Foo.java", 0, 10),
            named_sym("Bar@1", "Bar", None, "src/Bar.ts", 0, 5),
            named_sym("Bar@2", "Bar", None, "src/Bar.ts", 10, 15),
        ],
        vec![],
    );
    let index = HighlightIndex::new(&g);
    let r = |a: Anchor| resolve(&g, &index, &a);
    // Exact id first (a member lifts to its top-level symbol).
    assert_eq!(
        r(qualified("com.a.Foo#Foo", None, None)),
        found(&g, &["com.a.Foo"])
    );
    // A name: unique among top-level symbols (members do not count).
    assert_eq!(r(qualified("Foo", None, None)), Resolution::Ambiguous(2));
    assert_eq!(
        r(qualified("Foo", Some("a/Foo.java"), None)),
        found(&g, &["com.a.Foo"])
    );
    assert_eq!(
        r(qualified("Foo", Some("src/b/Foo.java"), None)),
        found(&g, &["com.b.Foo"])
    );
    // The hint matches whole path components only.
    assert_eq!(
        r(qualified("Foo", Some("Foo.java"), None)),
        Resolution::Ambiguous(2)
    );
    assert_eq!(
        r(qualified("Foo", Some("oo.java"), None)),
        Resolution::Unresolved
    );
    // The line narrows several candidates; one inside none is unresolved.
    assert_eq!(
        r(qualified("Bar", Some("Bar.ts"), Some(12))),
        found(&g, &["Bar@2"])
    );
    assert_eq!(r(qualified("Bar", None, Some(7))), Resolution::Unresolved);
    assert_eq!(r(qualified("Nope", None, None)), Resolution::Unresolved);
}

#[test]
fn propagate_callers_returns_non_seed_distances() {
    let (g, _) = fixture();
    let index = HighlightIndex::new(&g);
    let got = by_id(&g, propagate(&index, &idx(&g, &["Mapper"]), CALLERS));
    assert_eq!(
        got,
        BTreeMap::from([("Service".into(), 1), ("Action".into(), 2)])
    );
    // A seed reached from another seed stays out; distances are shortest.
    let got = by_id(
        &g,
        propagate(&index, &idx(&g, &["Mapper", "Service"]), CALLERS),
    );
    assert_eq!(got, BTreeMap::from([("Action".into(), 1)]));
    // No callers, no seeds: nothing.
    assert!(propagate(&index, &idx(&g, &["Lonely"]), CALLERS).is_empty());
    assert!(propagate(&index, &BTreeSet::new(), CALLERS).is_empty());
}

#[test]
fn propagate_callees_none_and_max_depth() {
    let (g, _) = fixture();
    let index = HighlightIndex::new(&g);
    let callees = Propagation {
        direction: Direction::Callees,
        max_depth: None,
    };
    let got = by_id(&g, propagate(&index, &idx(&g, &["Action"]), callees));
    // `ext:foo` is not a graph symbol.
    assert_eq!(
        got,
        BTreeMap::from([("Service".into(), 1), ("Mapper".into(), 2)])
    );
    let none = Propagation {
        direction: Direction::None,
        max_depth: None,
    };
    assert!(propagate(&index, &idx(&g, &["Mapper"]), none).is_empty());
    let depth = |max_depth| Propagation {
        direction: Direction::Callers,
        max_depth: Some(max_depth),
    };
    assert!(propagate(&index, &idx(&g, &["Mapper"]), depth(0)).is_empty());
    assert_eq!(
        by_id(&g, propagate(&index, &idx(&g, &["Mapper"]), depth(1))),
        BTreeMap::from([("Service".into(), 1)])
    );
    assert_eq!(
        by_id(&g, propagate(&index, &idx(&g, &["Mapper"]), depth(2))),
        BTreeMap::from([("Service".into(), 1), ("Action".into(), 2)])
    );
}

#[test]
fn propagate_terminates_on_cycles() {
    let g = graph(
        vec![
            sym("A", None, "A.java", 0, 5),
            sym("B", None, "B.java", 0, 5),
            sym("C", None, "C.java", 0, 5),
        ],
        vec![
            edge("A", "B", EdgeKind::Call),
            edge("B", "C", EdgeKind::Call),
            edge("C", "A", EdgeKind::Call),
            edge("A", "A", EdgeKind::Call),
        ],
    );
    let index = HighlightIndex::new(&g);
    assert_eq!(
        by_id(&g, propagate(&index, &idx(&g, &["A"]), CALLERS)),
        BTreeMap::from([("C".into(), 1), ("B".into(), 2)])
    );
    let callees = Propagation {
        direction: Direction::Callees,
        max_depth: None,
    };
    assert_eq!(
        by_id(&g, propagate(&index, &idx(&g, &["A"]), callees)),
        BTreeMap::from([("B".into(), 1), ("C".into(), 2)])
    );
}

fn hs(id: &str, role: Role, distance: u32) -> HighlightedSymbol {
    HighlightedSymbol {
        id: id.into(),
        role,
        distance,
    }
}

fn hc(id: &str, role: Role, distance: u32) -> HighlightedCell {
    HighlightedCell {
        id: id.into(),
        role,
        distance,
    }
}

#[test]
fn aggregate_maps_symbols_to_cells() {
    let (_, c) = fixture();
    let got = aggregate(
        &c,
        &[
            hs("Mapper", Role::Changed, 0),
            hs("Iface", Role::Changed, 0),
            hs("Lonely", Role::Changed, 0),
            hs("Action", Role::Reached, 3),
            hs("Service", Role::Reached, 2),
            hs("Impl", Role::Reached, 1),
            hs("User", Role::Reached, 1),
            hs("Service", Role::Reached, 1),
        ],
    );
    // Impl shares a changed cell; User and Lonely are unplaced; the min
    // distance wins.
    assert_eq!(
        got,
        [
            hc("business:b", Role::Changed, 0),
            hc("persistence:a", Role::Changed, 0),
            hc("business:a", Role::Reached, 1),
            hc("entry:a", Role::Reached, 3),
        ]
    );
    // A reached seed at distance 0 sorts after a changed one.
    assert_eq!(
        aggregate(
            &c,
            &[
                hs("Action", Role::Reached, 0),
                hs("Mapper", Role::Changed, 0)
            ]
        ),
        [
            hc("persistence:a", Role::Changed, 0),
            hc("entry:a", Role::Reached, 0),
        ]
    );
    assert!(aggregate(&c, &[]).is_empty());
}

#[test]
fn highlight_composes_the_steps() {
    let (g, c) = fixture();
    let index = HighlightIndex::new(&g);
    let source = SourceInfo {
        kind: "test".into(),
        label: "a test".into(),
    };
    let seed = |anchor: Anchor, role| Seed { anchor, role };
    let request = HighlightRequest {
        source: source.clone(),
        seeds: vec![
            seed(loc("Mapper.java", vec![(6, 7)]), Role::Reached),
            seed(Anchor::Symbol { id: "Nope".into() }, Role::Changed),
            seed(
                Anchor::Symbol {
                    id: "Mapper#map".into(),
                },
                Role::Changed,
            ),
            seed(qualified("Lonely", None, None), Role::Reached),
            seed(loc("Gone.java", vec![]), Role::Changed),
        ],
        propagate: CALLERS,
    };
    let h = highlight(&g, &c, &index, &request);
    assert_eq!(h.source, source);
    // Mapper is seeded twice: the strongest role wins.
    assert_eq!(
        h.symbols,
        [
            hs("Mapper", Role::Changed, 0),
            hs("Lonely", Role::Reached, 0),
            hs("Service", Role::Reached, 1),
            hs("Action", Role::Reached, 2),
        ]
    );
    assert_eq!(
        h.cells,
        [
            hc("persistence:a", Role::Changed, 0),
            hc("business:a", Role::Reached, 1),
            hc("entry:a", Role::Reached, 2),
        ]
    );
    assert_eq!(
        h.unresolved,
        [
            Anchor::Symbol { id: "Nope".into() },
            loc("Gone.java", vec![])
        ]
    );
    assert!(h.warnings.is_empty());
    // Same request, same output.
    assert_eq!(highlight(&g, &c, &index, &request), h);
}

#[test]
fn highlight_warns_on_ambiguous_names() {
    let g = graph(
        vec![
            named_sym("a.Foo", "Foo", None, "a/Foo.java", 0, 5),
            named_sym("b.Foo", "Foo", None, "b/Foo.java", 0, 5),
        ],
        vec![],
    );
    let request = HighlightRequest {
        source: SourceInfo {
            kind: "test".into(),
            label: String::new(),
        },
        seeds: vec![Seed {
            anchor: qualified("Foo", None, None),
            role: Role::Changed,
        }],
        propagate: CALLERS,
    };
    let h = highlight(
        &g,
        &Classification::default(),
        &HighlightIndex::new(&g),
        &request,
    );
    assert!(h.symbols.is_empty());
    assert_eq!(h.unresolved, [qualified("Foo", None, None)]);
    assert_eq!(h.warnings, ["ambiguous name Foo: 2 candidates"]);
}

#[test]
fn default_index_highlights_nothing() {
    let (g, _) = fixture();
    let index = HighlightIndex::default();
    assert!(propagate(&index, &BTreeSet::from([0]), CALLERS).is_empty());
    assert_eq!(
        resolve(
            &g,
            &index,
            &Anchor::Symbol {
                id: "Mapper".into()
            }
        ),
        Resolution::Unresolved
    );
}

#[test]
fn request_json_shape() {
    let json = r#"{"source":{"kind":"git-diff","label":"HEAD"},"seeds":[
        {"anchor":{"kind":"symbol","id":"A"},"role":"changed"},
        {"anchor":{"kind":"location","file":"A.java","lines":[[0,2]]},"role":"reached"},
        {"anchor":{"kind":"qualified","name":"A"},"role":"changed"}],
        "propagate":{"direction":"callers"}}"#;
    let r: HighlightRequest = serde_json::from_str(json).unwrap();
    assert_eq!(r.seeds[1].anchor, loc("A.java", vec![(0, 2)]));
    assert_eq!(r.seeds[2].anchor, qualified("A", None, None));
    assert_eq!(r.propagate, CALLERS);
}
