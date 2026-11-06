use std::path::PathBuf;
use std::time::Instant;

use structure_core::ir::{EdgeKind, SymbolKind};
use structure_core::ts::parse_project;

#[test]
fn angular_realworld() {
    let fixtures = std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let root = fixtures.join("angular-realworld");
    if !root.is_dir() {
        eprintln!("skipping: {} not found", root.display());
        return;
    }
    let t = Instant::now();
    let g = parse_project(&root).expect("parse");
    eprintln!(
        "ts parse: {} symbols, {} edges in {:?}",
        g.symbols.len(),
        g.edges.len(),
        t.elapsed()
    );
    let with = |a: &str| {
        g.symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class && s.annotations.iter().any(|x| x == a))
            .count()
    };
    assert!(with("Component") >= 3, "components");
    assert!(with("Injectable") >= 2, "injectables");
    assert!(
        g.edges
            .iter()
            .any(|e| e.kind == EdgeKind::Uses && e.to == "ext:HttpClient"),
        "Uses ext:HttpClient"
    );
}
