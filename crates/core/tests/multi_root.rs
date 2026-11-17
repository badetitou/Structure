//! Several roots analysed together (tree-sitter only). Skips when the
//! fixtures are missing. Indexers are never run here.

use std::path::{Path, PathBuf};

use structure_core::ir::{EdgeKind, protocol};
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
fn http_calls_link_to_backend_handlers() {
    let (Some(back), Some(front)) = (
        fixture("spring-boot-realworld-example-app"),
        fixture("angular-realworld"),
    ) else {
        return;
    };
    let a = pipeline::analyze_many(&[back, front], &offline(), &|_| {}).expect("analyze");
    let s = a.http_stats().expect("http stats");
    assert_eq!(
        (s.calls, s.linked, s.unknown_path, s.ambiguous),
        (18, 17, 1, 0),
        "{s:?}"
    );
    assert!(s.unmatched.is_empty(), "{s:?}");
    let has = |from: &str, to: &str| {
        a.graph.edges.iter().any(|e| {
            e.kind == EdgeKind::remote(protocol::HTTP)
                && e.from.ends_with(from)
                && e.to.ends_with(to)
        })
    };
    assert!(has(
        "UserService.getCurrentUser",
        "CurrentUserApi#currentUser(2)"
    ));
    assert!(has(
        "CommentsService.delete",
        "CommentsApi#deleteComment(3)"
    ));
    assert!(has("ArticlesService.get", "ArticleApi#article(2)"), "{s:?}");
    assert!(
        a.matrix.http_edges.iter().any(|g| {
            let from = &a.matrix.cells[g.from].id;
            from.starts_with("api_client:angular-realworld")
                && g.targets
                    .iter()
                    .any(|&(t, _)| a.matrix.cells[t].id.starts_with("entry:spring-boot"))
        }),
        "{:?}",
        a.matrix.http_edges
    );
}

#[test]
fn one_root_is_unchanged() {
    let Some(root) = fixture("angular-realworld") else {
        return;
    };
    let single = pipeline::analyze(&root, &offline(), &|_| {}).expect("analyze");
    let many = pipeline::analyze_many(&[root], &offline(), &|_| {}).expect("analyze");
    // Timings differ between runs; everything else must be identical.
    let json = |mut m: structure_core::model::Matrix| {
        m.stats.elapsed_ms = 0;
        serde_json::to_string(&m).expect("serialize")
    };
    let (single, many) = (json(single.matrix), json(many.matrix));
    assert_eq!(single, many);
    assert!(!many.contains("\"projects\""));
}
