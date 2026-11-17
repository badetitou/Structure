//! One root holding a Java backend and a TypeScript frontend: analysed as two
//! `(root, language)` units merged like two roots (tree-sitter only, no
//! indexer run).

use std::path::{Path, PathBuf};

use structure_core::ir::{EdgeKind, Language, protocol};
use structure_core::pipeline::{self, Options};
use structure_core::session::Session;

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn offline() -> Options {
    Options {
        use_scip: false,
        ..Options::default()
    }
}

const CONTROLLER: &str = "src/main/java/com/x/web/UserController.java";
const SERVICE: &str = "web/src/app/user.service.ts";

/// A git repository whose root `app/` holds a Maven backend and an Angular
/// frontend (one commit).
fn mixed_repo(tag: &str) -> (PathBuf, PathBuf) {
    let repo = std::env::temp_dir().join(format!("structure-ml-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    let root = repo.join("app");
    std::fs::create_dir_all(&root).unwrap();
    git(&repo, &["init", "-q"]);
    write(&root, "pom.xml", "<project/>");
    write(&root, "package.json", "{}");
    write(
        &root,
        CONTROLLER,
        "package com.x.web;\n\
         @RestController\n\
         @RequestMapping(\"/api/users\")\n\
         public class UserController {\n\
         \x20 @GetMapping(\"/{id}\")\n\
         \x20 public String get(@PathVariable String id) { return id; }\n\
         }\n",
    );
    write(
        &root,
        SERVICE,
        "import { HttpClient } from '@angular/common/http';\n\
         export class UserService {\n\
         \x20 constructor(private http: HttpClient) {}\n\
         \x20 get(id: string) { return this.http.get(`/api/users/${id}`); }\n\
         }\n",
    );
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "c0"]);
    (repo, root)
}

#[test]
fn detection_lists_languages_with_sources() {
    let (repo, root) = mixed_repo("detect");
    assert_eq!(
        pipeline::detect_languages(&root).unwrap(),
        [Language::Java, Language::TypeScript]
    );
    // A package.json kept for tooling only: Java alone, as before.
    std::fs::remove_dir_all(root.join("web")).unwrap();
    assert_eq!(pipeline::detect_languages(&root).unwrap(), [Language::Java]);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn one_root_two_languages_are_two_projects_linked_over_http() {
    let (repo, root) = mixed_repo("analyze");
    let a = pipeline::analyze(&root, &offline(), &|_| {}).expect("analyze");
    let projects: Vec<(&str, Language, &str)> = a
        .matrix
        .projects
        .iter()
        .map(|p| (p.name.as_str(), p.language, p.root.as_str()))
        .collect();
    let r = root.display().to_string();
    assert_eq!(
        projects,
        [
            ("app (java)", Language::Java, r.as_str()),
            ("app (typescript)", Language::TypeScript, r.as_str())
        ]
    );
    assert_eq!(a.roots, [root.clone(), root.clone()]);
    assert_eq!(a.matrix.project, "app (java)");
    // Each unit only reads its own language's sources.
    for s in &a.graph.symbols {
        if s.id.starts_with("app (java)::") {
            assert!(
                s.file.starts_with("app (java)/") && s.file.ends_with(".java"),
                "{s:?}"
            );
        } else {
            assert!(s.id.starts_with("app (typescript)::"), "{s:?}");
            assert!(
                s.file.starts_with("app (typescript)/") && s.file.ends_with(".ts"),
                "{s:?}"
            );
        }
    }
    assert!(a.matrix.cells.iter().all(|c| c.domain.starts_with("app (")));
    // The linkers ran on the merged graph: same-repo TS -> Java HTTP edge.
    assert!(
        a.graph
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::remote(protocol::HTTP)
                && e.from.starts_with("app (typescript)::")
                && e.from.contains("UserService")
                && e.to.starts_with("app (java)::")
                && e.to.contains("UserController")),
        "{:?}",
        a.graph.edges
    );
    assert_eq!(a.http_stats().map(|s| s.linked), Some(1));
    // Same result through the multi-root entry point.
    let many = pipeline::analyze_many(std::slice::from_ref(&root), &offline(), &|_| {}).unwrap();
    assert_eq!(many.matrix.cells.len(), a.matrix.cells.len());
    assert_eq!(many.matrix.projects, a.matrix.projects);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn reclassify_and_rules_stamp_cover_both_languages() {
    let (repo, root) = mixed_repo("rules");
    let a = pipeline::analyze(&root, &offline(), &|_| {}).expect("analyze");
    let same = pipeline::reclassify(&a).expect("reclassify");
    assert_eq!(same.matrix.cells, a.matrix.cells);
    assert_eq!(same.matrix.projects, a.matrix.projects);
    assert_eq!(
        pipeline::rules_stamp_of(&a.roots),
        pipeline::rules_stamp(std::slice::from_ref(&root)).unwrap()
    );
    // The root's rules apply to both units.
    write(
        &root,
        ".structure/rules.toml",
        "[[layer]]\nid = \"svc\"\nlabel = \"Services\"\norder = 0\n\
         [[rule]]\nlayer = \"svc\"\nname_suffix = [\"Controller\", \"Service\"]\n",
    );
    let b = pipeline::reclassify(&a).expect("reclassify");
    let layer_of = |name: &str| {
        let s = a
            .graph
            .symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name}"));
        b.classification.placements[&s.id].layer.to_string()
    };
    assert_eq!(layer_of("UserController"), "svc");
    assert_eq!(layer_of("UserService"), "svc");
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn impact_maps_changed_files_to_their_unit() {
    let (repo, root) = mixed_repo("impact");
    let c0 = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    write(
        &root,
        CONTROLLER,
        "package com.x.web;\n\
         @RestController\n\
         @RequestMapping(\"/api/users\")\n\
         public class UserController {\n\
         \x20 @GetMapping(\"/{id}\")\n\
         \x20 public String get(@PathVariable String id) { return id + \"!\"; }\n\
         }\n",
    );
    // Working tree: the cached analysis is merged, the diff is root-relative.
    let s = Session::with_impact_options(offline());
    let path = root.display().to_string();
    let m = s
        .analyze(std::slice::from_ref(&path), &offline(), &|_| {})
        .unwrap();
    assert_eq!(m.projects.len(), 2);
    let imp = s.impact(&path, None, None, &|_| {}).expect("impact");
    let java_changed = |ids: &[String]| {
        !ids.is_empty()
            && ids
                .iter()
                .all(|id| id.starts_with("app (java)::") && id.contains("UserController"))
    };
    assert_eq!(imp.changed_files, [CONTROLLER]);
    assert!(java_changed(&imp.changed_symbols), "{imp:?}");
    assert!(
        imp.impacted_symbols
            .iter()
            .any(|s| s.id.starts_with("app (typescript)::") && s.id.contains("UserService")),
        "the HTTP caller is impacted: {imp:?}"
    );
    // A commit: analysed in its snapshot, also as two units.
    git(&repo, &["commit", "-q", "-am", "c1"]);
    let imp = s
        .impact(&path, Some(&c0), Some("HEAD"), &|_| {})
        .expect("impact at commit");
    assert_eq!(imp.changed_files, [CONTROLLER]);
    assert!(java_changed(&imp.changed_symbols), "{imp:?}");
    assert_eq!(imp.matrix.map(|m| m.projects.len()), Some(2));
    let _ = std::fs::remove_dir_all(&repo);
}
