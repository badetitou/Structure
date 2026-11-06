use std::path::PathBuf;
use std::time::Instant;

use structure_core::ir::{EdgeKind, Graph};
use structure_core::link::link;
use structure_core::matrix::class_edges;
use structure_core::scip;

fn base() -> PathBuf {
    std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"))
}

/// Parse, link, print stats, check the mapping ratio. None if fixture/index missing.
fn run(name: &str, parse: fn(&std::path::Path) -> anyhow::Result<Graph>) -> Option<Graph> {
    let root = base().join(name);
    let idx_path = root.join("index.scip");
    if !idx_path.exists() {
        eprintln!("skipping {name}: {} not found", idx_path.display());
        return None;
    }
    let g = parse(&root).expect("parse");
    let index = scip::load(&idx_path).expect("load scip");
    let t = Instant::now();
    let (g, st) = link(g, &index);
    // Share of the SCIP definitions in parsed files that land on an ir symbol,
    // directly or through an enclosing one.
    let resolved = st.mapped_symbols + st.lifted_symbols;
    let ratio = resolved as f64 / (resolved + st.unmapped_definitions).max(1) as f64;
    eprintln!("{name}: {st:?} ratio={ratio:.3} in {:?}", t.elapsed());
    assert!(ratio > 0.8, "{name}: mapping ratio {ratio:.3} too low");
    Some(g)
}

fn top_id(g: &Graph, name: &str, file_suffix: &str) -> String {
    g.symbols
        .iter()
        .find(|s| s.container.is_none() && s.name == name && s.file.ends_with(file_suffix))
        .unwrap_or_else(|| panic!("{name} not found"))
        .id
        .clone()
}

fn has_class_edge(g: &Graph, from: &str, to: &str) {
    let edges = class_edges(g);
    assert!(
        edges.iter().any(|(f, t, _, _)| f == from && t == to),
        "missing class edge {from} -> {to}"
    );
}

#[test]
fn jpetstore_link() {
    let Some(g) = run("jpetstore-6", structure_core::java::parse_project) else {
        return;
    };
    let ctl = "org.mybatis.jpetstore.web.controllers.AccountController";
    let svc = "org.mybatis.jpetstore.service.AccountService";
    let mapper = "org.mybatis.jpetstore.mapper.AccountMapper";
    has_class_edge(&g, ctl, svc);
    has_class_edge(&g, svc, mapper);
}

#[test]
fn realworld_link() {
    let Some(g) = run("angular-realworld", structure_core::ts::parse_project) else {
        return;
    };
    let comp = top_id(&g, "FollowButtonComponent", "follow-button.component.ts");
    let svc = top_id(&g, "ProfileService", "profile.service.ts");
    has_class_edge(&g, &comp, &svc);
    assert!(
        g.edges
            .iter()
            .any(|e| e.to == "ext:HttpClient" && e.from.starts_with(&svc)),
        "ProfileService should reach ext:HttpClient"
    );
    assert!(g.edges.iter().any(|e| e.kind == EdgeKind::Call));
}
