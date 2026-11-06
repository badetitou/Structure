use std::path::PathBuf;
use std::time::Instant;

use structure_core::java::parse_project;

#[test]
fn jpetstore_symbols() {
    let base = std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let root = base.join("jpetstore-6");
    if !root.exists() {
        eprintln!("skipping: {} not found", root.display());
        return;
    }
    let t = Instant::now();
    let g = parse_project(&root).expect("parse");
    eprintln!(
        "parsed {} symbols, {} edges in {:?}",
        g.symbols.len(),
        g.edges.len(),
        t.elapsed()
    );

    let find = |suffix: &str| {
        g.symbols
            .iter()
            .find(|s| s.id.ends_with(suffix) && s.container.is_none())
            .unwrap_or_else(|| panic!("{suffix} not found"))
    };
    let svc = find(".AccountService");
    assert_eq!(
        svc.package.as_deref(),
        Some("org.mybatis.jpetstore.service")
    );
    assert!(svc.annotations.iter().any(|a| a == "Service"));
    let bean = find(".AccountController");
    assert_eq!(
        bean.package.as_deref(),
        Some("org.mybatis.jpetstore.web.controllers")
    );
    assert!(bean.annotations.iter().any(|a| a == "Controller"));
    let mapper = find(".AccountMapper");
    assert_eq!(
        mapper.package.as_deref(),
        Some("org.mybatis.jpetstore.mapper")
    );
    let acc = find(".Account");
    assert_eq!(acc.package.as_deref(), Some("org.mybatis.jpetstore.domain"));
}
