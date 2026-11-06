//! End-to-end tests on real fixtures. They skip (with a note) when the
//! fixture, its `index.scip` or the golden file is missing. Indexers are
//! never run here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use structure_core::ir::EdgeKind;
use structure_core::matrix::class_edges;
use structure_core::pipeline::{self, Analysis, Options};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace().join("fixtures"))
}

fn offline() -> Options {
    Options {
        reindex: false,
        reindex_if_stale: false,
        use_scip: true,
    }
}

fn fixture(name: &str) -> Option<PathBuf> {
    let root = fixtures().join(name);
    if !root.join("index.scip").is_file() {
        eprintln!("skipping {name}: {}/index.scip not found", root.display());
        return None;
    }
    Some(root)
}

fn golden(name: &str) -> Option<toml::Value> {
    let candidates = [
        workspace()
            .join("tests/golden")
            .join(format!("{name}.toml")),
        fixtures()
            .join("../tests/golden")
            .join(format!("{name}.toml")),
    ];
    let Some(p) = candidates.iter().find(|p| p.is_file()) else {
        eprintln!("skipping {name}: golden file not found");
        return None;
    };
    let text = std::fs::read_to_string(p).expect("read golden");
    Some(toml::from_str(&text).expect("parse golden"))
}

fn check(name: &str, fixture_dir: &str) {
    let Some(root) = fixture(fixture_dir) else {
        return;
    };
    let t = Instant::now();
    let a: Analysis = pipeline::analyze(&root, &offline(), &|_| {}).expect("analyze");
    let total = t.elapsed();
    eprintln!(
        "{name}: analyze {total:?}; {} symbols, {} edges, link {:?}, warnings {:?}",
        a.graph.symbols.len(),
        a.graph.edges.len(),
        a.link_stats,
        a.warnings
    );
    assert!(a.link_stats.is_some(), "{name}: index.scip was not linked");
    assert!(total.as_secs() < 10, "{name}: analysis too slow: {total:?}");

    let Some(g) = golden(name) else {
        return;
    };
    let classes = g
        .get("class")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let (mut layer_ok, mut domain_ok, mut n) = (0usize, 0usize, 0usize);
    for c in &classes {
        let get = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or("");
        let Some(p) = a.classification.placements.get(get("name")) else {
            eprintln!("  golden class not found: {}", get("name"));
            n += 1;
            continue;
        };
        n += 1;
        if p.layer.as_str() == get("layer") {
            layer_ok += 1;
        } else {
            eprintln!(
                "  layer mismatch {}: got {} want {}",
                get("name"),
                p.layer.as_str(),
                get("layer")
            );
        }
        if p.domain == get("domain") {
            domain_ok += 1;
        } else {
            eprintln!(
                "  domain mismatch {}: got {} want {}",
                get("name"),
                p.domain,
                get("domain")
            );
        }
    }
    let n_f = n.max(1) as f64;
    let layer_acc = layer_ok as f64 / n_f;
    eprintln!(
        "{name}: layer accuracy {layer_acc:.3} ({layer_ok}/{n}), domain accuracy {:.3} ({domain_ok}/{n})",
        domain_ok as f64 / n_f
    );
    assert!(n > 0, "{name}: golden has no classes");
    assert!(
        layer_acc >= 0.8,
        "{name}: layer accuracy {layer_acc:.3} < 0.8"
    );

    let edges: HashMap<(String, String), bool> = class_edges(&a.graph)
        .into_iter()
        .filter(|(_, _, k, _)| matches!(k, EdgeKind::Call | EdgeKind::Uses))
        .map(|(f, t, _, _)| ((f, t), true))
        .collect();
    let paths = g
        .get("path")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    for p in &paths {
        let chain: Vec<String> = p
            .get("chain")
            .and_then(|c| c.as_array())
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        for w in chain.windows(2) {
            assert!(
                edges.contains_key(&(w[0].clone(), w[1].clone())),
                "{name}: missing class edge {} -> {}",
                w[0],
                w[1]
            );
        }
    }
}

#[test]
fn e2e_jpetstore() {
    check("jpetstore", "jpetstore-6");
}

#[test]
fn e2e_realworld() {
    check("realworld", "angular-realworld");
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git");
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn e2e_jpetstore_impact() {
    let Some(src) = fixture("jpetstore-6") else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!("structure-e2e-impact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    let clone = tmp.join("jpetstore-6");
    let o = Command::new("git")
        .args(["clone", "--local", "-q"])
        .arg(&src)
        .arg(&clone)
        .output()
        .expect("git clone");
    if !o.status.success() {
        eprintln!(
            "skipping impact e2e: clone failed: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        return;
    }
    std::fs::copy(src.join("index.scip"), clone.join("index.scip")).expect("copy index");
    git(&clone, &["config", "user.name", "t"]);

    let mapper = clone.join("src/main/java/org/mybatis/jpetstore/mapper/AccountMapper.java");
    let text = std::fs::read_to_string(&mapper).expect("read mapper");
    std::fs::write(
        &mapper,
        text.replacen(
            "interface AccountMapper {",
            "interface AccountMapper {\n  // changed\n",
            1,
        ),
    )
    .expect("write mapper");

    let a = pipeline::analyze(&clone, &offline(), &|_| {}).expect("analyze");
    let imp = pipeline::impact(&clone, &a, None, None).expect("impact");
    eprintln!("impact: {imp:?}");

    let dist = |suffix: &str| {
        imp.impacted_symbols
            .iter()
            .find(|s| s.id.ends_with(suffix))
            .map(|s| s.distance)
    };
    assert_eq!(dist("service.AccountService"), Some(1));
    assert_eq!(dist("controllers.AccountController"), Some(2));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn detect_language_errors_clearly() {
    let d = std::env::temp_dir().join(format!("structure-detect-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    let e = pipeline::detect_language(&d).expect_err("no marker");
    assert!(e.to_string().contains("cannot detect language"));
    std::fs::write(d.join("pom.xml"), "").expect("write");
    assert_eq!(
        pipeline::detect_language(&d).expect("java"),
        structure_core::ir::Language::Java
    );
    let _ = std::fs::remove_dir_all(&d);
}
