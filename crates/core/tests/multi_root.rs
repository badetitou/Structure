//! Several roots analysed together (tree-sitter only). Skips when the
//! fixtures are missing. Indexers are never run here.

use std::path::{Path, PathBuf};

use structure_core::pipeline::{self, Analysis, Options};

fn fixtures() -> PathBuf {
    std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"))
}

fn fixture(name: &str) -> Option<PathBuf> {
    let root = fixtures().join(name);
    if !root.is_dir() {
        eprintln!("skipping {name}: {} not found", root.display());
        return None;
    }
    Some(root)
}

fn offline() -> Options {
    Options {
        reindex: false,
        reindex_if_stale: false,
        // Tree-sitter only: no indexer, no `index.scip` needed.
        use_scip: false,
        ..Options::default()
    }
}

#[test]
fn two_roots_give_the_union_of_both_cell_sets() {
    let (Some(back), Some(front)) = (
        fixture("spring-boot-realworld-example-app"),
        fixture("angular-realworld"),
    ) else {
        return;
    };
    let one = |r: &Path| pipeline::analyze(r, &offline(), &|_| {}).expect("analyze");
    let (b, f) = (one(&back), one(&front));
    let both = pipeline::analyze_many(&[back, front], &offline(), &|_| {}).expect("analyze");
    let m = &both.matrix;
    let projects: Vec<_> = m.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        projects,
        ["spring-boot-realworld-example-app", "angular-realworld"]
    );
    let cells = |p: &str, a: &Analysis| -> Vec<(String, usize)> {
        a.matrix
            .cells
            .iter()
            .map(|c| (format!("{}:{p}/{}", c.layer.as_str(), c.domain), c.size))
            .collect()
    };
    let mut want = cells("spring-boot-realworld-example-app", &b);
    want.extend(cells("angular-realworld", &f));
    want.sort();
    let mut got: Vec<(String, usize)> = m.cells.iter().map(|c| (c.id.clone(), c.size)).collect();
    got.sort();
    assert_eq!(got, want);
    assert_eq!(
        both.graph.symbols.len(),
        b.graph.symbols.len() + f.graph.symbols.len()
    );
}

#[test]
fn one_root_is_unchanged() {
    let Some(root) = fixture("angular-realworld") else {
        return;
    };
    let single = pipeline::analyze(&root, &offline(), &|_| {}).expect("analyze");
    let many = pipeline::analyze_many(&[root], &offline(), &|_| {}).expect("analyze");
    assert!(many.matrix.projects.is_empty());
    assert_eq!(single.matrix.cells, many.matrix.cells);
    assert_eq!(single.matrix.domains, many.matrix.domains);
}
