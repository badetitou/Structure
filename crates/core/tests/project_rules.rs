//! `<project>/.structure/rules.toml`: layers merged by id over the default
//! rules (and across roots, the first root winning), invalid files rejected
//! with their path (tree-sitter only).

use std::path::{Path, PathBuf};

use structure_core::pipeline::{self, Analysis, Options};

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn project(name: &str, rules: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "structure_project_rules_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    write(&dir, "pom.xml", "<project/>");
    write(
        &dir,
        "src/main/java/a/Page.java",
        "package a; @Controller public class Page {}",
    );
    write(
        &dir,
        "src/main/java/a/LoginView.java",
        "package a; public class LoginView { void show() { new Page(); } }",
    );
    write(&dir, ".structure/rules.toml", rules);
    dir
}

/// `(id, label, order)` of each matrix layer.
fn layer_rows(a: &Analysis) -> Vec<(String, String, u32)> {
    a.matrix
        .layers
        .iter()
        .map(|d| (d.id.to_string(), d.label.clone(), d.order))
        .collect()
}

fn row(id: &str, label: &str, order: u32) -> (String, String, u32) {
    (id.to_string(), label.to_string(), order)
}

/// Layer id of the top-level symbol named `name`.
fn layer_of(a: &Analysis, name: &str) -> String {
    let sym = a
        .graph
        .symbols
        .iter()
        .find(|s| s.name == name && a.classification.placements.contains_key(&s.id))
        .unwrap_or_else(|| panic!("no placed symbol {name}"));
    a.classification.placements[&sym.id].layer.to_string()
}

fn offline() -> Options {
    Options {
        use_scip: false,
        ..Options::default()
    }
}

#[test]
fn project_rules_add_and_relabel_layers() {
    let dir = project(
        "ok",
        "[[layer]]\nid = \"view\"\nlabel = \"Views\"\norder = 0\n\
         [[layer]]\nid = \"entry\"\nlabel = \"Entry points\"\norder = 3\n\
         [[rule]]\nlayer = \"view\"\nname_suffix = [\"View\"]\n",
    );
    let a = pipeline::analyze(&dir, &offline(), &|_| {}).expect("analyze");
    assert_eq!(
        layer_rows(&a),
        [row("view", "Views", 0), row("entry", "Entry points", 3)]
    );
    // The project's own rule places LoginView; the default one places Page.
    assert_eq!(layer_of(&a, "LoginView"), "view");
    assert_eq!(layer_of(&a, "Page"), "entry");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_rule_on_undeclared_layer_names_the_file_and_layer() {
    let dir = project(
        "bad",
        "[[rule]]\nlayer = \"view\"\nname_suffix = [\"View\"]\n",
    );
    let err = format!(
        "{:#}",
        pipeline::analyze(&dir, &offline(), &|_| {}).unwrap_err()
    );
    assert!(err.contains(".structure/rules.toml"), "{err}");
    assert!(err.contains("undeclared layer `view`"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_layer_declared_twice_is_rejected() {
    let decl = "[[layer]]\nid = \"view\"\nlabel = \"Views\"\norder = 0\n";
    let dir = project("twice", &format!("{decl}{decl}"));
    let err = format!(
        "{:#}",
        pipeline::analyze(&dir, &offline(), &|_| {}).unwrap_err()
    );
    assert!(err.contains(".structure/rules.toml"), "{err}");
    assert!(err.contains("layer `view` is declared twice"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unparsable_project_rules_name_the_file() {
    let dir = project("syntax", "[[layer]\nid = \"view\"\n");
    let err = format!(
        "{:#}",
        pipeline::analyze(&dir, &offline(), &|_| {}).unwrap_err()
    );
    assert!(err.contains(".structure/rules.toml"), "{err}");
    assert!(err.contains("invalid classification rules"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_field_in_project_rules_is_rejected() {
    let dir = project(
        "field",
        "[[rule]]\nlayer = \"entry\"\nname_sufix = [\"View\"]\n",
    );
    let err = format!(
        "{:#}",
        pipeline::analyze(&dir, &offline(), &|_| {}).unwrap_err()
    );
    assert!(err.contains(".structure/rules.toml"), "{err}");
    assert!(err.contains("unknown field `name_sufix`"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_rules_of_a_later_root_fail_the_multi_root_analysis() {
    let good = project("multi_good", "");
    let bad = project("multi_bad", "[[rule]]\nlayer = \"view\"\n");
    let err = format!(
        "{:#}",
        pipeline::analyze_many(&[good.clone(), bad.clone()], &offline(), &|_| {}).unwrap_err()
    );
    assert!(err.contains("multi_bad"), "{err}");
    assert!(err.contains(".structure/rules.toml"), "{err}");
    assert!(err.contains("undeclared layer `view`"), "{err}");
    let _ = std::fs::remove_dir_all(&good);
    let _ = std::fs::remove_dir_all(&bad);
}

/// Shared ids take the first root's label and order; layers only declared
/// by a later root come after the first root's, before `unknown`.
#[test]
fn layers_merge_by_id_across_roots_first_root_wins() {
    let first = project(
        "multi_a",
        "[[layer]]\nid = \"entry\"\nlabel = \"First\"\norder = 3\n",
    );
    let second = project(
        "multi_b",
        "[[layer]]\nid = \"view\"\nlabel = \"Views\"\norder = 0\n\
         [[layer]]\nid = \"entry\"\nlabel = \"Second\"\norder = 1\n\
         [[rule]]\nlayer = \"view\"\nname_suffix = [\"View\"]\n",
    );
    let a = pipeline::analyze_many(&[first.clone(), second.clone()], &offline(), &|_| {})
        .expect("analyze");
    // The default layers end at `util` (order 7): `view` is appended after it.
    assert_eq!(
        layer_rows(&a),
        [
            row("entry", "First", 3),
            row("view", "Views", 8),
            row("unknown", "unknown", 9)
        ]
    );
    // Swapped roots: the other project is now primary.
    let b = pipeline::analyze_many(&[second.clone(), first.clone()], &offline(), &|_| {})
        .expect("analyze");
    assert_eq!(
        layer_rows(&b),
        [
            row("view", "Views", 0),
            row("entry", "Second", 1),
            row("unknown", "unknown", 8)
        ]
    );
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}
