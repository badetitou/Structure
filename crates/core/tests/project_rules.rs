//! `<project>/.structure/rules.toml`: layers merged by id over the default
//! rules, undeclared layers rejected (tree-sitter only).

use std::path::{Path, PathBuf};

use structure_core::pipeline::{self, Options};

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
    let rows: Vec<(&str, &str)> = a
        .matrix
        .layers
        .iter()
        .map(|d| (d.id.as_str(), d.label.as_str()))
        .collect();
    assert_eq!(rows, [("view", "Views"), ("entry", "Entry points")]);
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
