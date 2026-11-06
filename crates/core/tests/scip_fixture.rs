use std::path::PathBuf;
use std::time::Instant;

use structure_core::scip;

fn fixtures_dir() -> PathBuf {
    match std::env::var_os("STRUCTURE_FIXTURES") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"),
    }
}

fn check(rel: &str, ext: &str) {
    let path = fixtures_dir().join(rel);
    if !path.exists() {
        eprintln!("skipping: {} not found", path.display());
        return;
    }
    let t = Instant::now();
    let idx = scip::load(&path).expect("load index");
    let (d, s, o) = idx.stats();
    eprintln!(
        "{rel}: {d} docs, {s} symbols, {o} occurrences in {:?}",
        t.elapsed()
    );
    assert!(d > 0 && s > 0 && o > 0);
    let has_def = idx
        .documents
        .iter()
        .filter(|doc| doc.relative_path.ends_with(ext))
        .any(|doc| doc.occurrences.iter().any(|x| x.is_definition));
    assert!(has_def, "no definition occurrence in a {ext} document");
    let mut n = 0;
    let mut enclosing = 0;
    for doc in &idx.documents {
        for x in doc.occurrences.iter().filter(|x| x.is_definition) {
            if n < 3 {
                eprintln!("  sample {} {:?} {}", doc.relative_path, x.range, x.symbol);
                n += 1;
            }
            enclosing += usize::from(x.enclosing_range.is_some());
        }
    }
    eprintln!("  definitions with enclosing_range: {enclosing}");
}

#[test]
fn jpetstore() {
    check("jpetstore-6/index.scip", ".java");
}

#[test]
fn angular_realworld() {
    check("angular-realworld/index.scip", ".ts");
}
