//! GWT RPC edges end to end on a minimal layout (tree-sitter only).

use std::path::{Path, PathBuf};

use structure_core::ir::EdgeKind;
use structure_core::pipeline::{self, Options};

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("structure_gwt_core_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write(&dir, "pom.xml", "<project/>");
    for (rel, src) in files {
        write(&dir, &format!("src/main/java/{rel}"), src);
    }
    dir
}

fn offline() -> Options {
    Options {
        use_scip: false,
        ..Options::default()
    }
}

const SERVICE: (&str, &str) = (
    "c/ISvc.java",
    "package c; public interface ISvc extends RemoteService { String go(int a); }",
);
const ASYNC: (&str, &str) = (
    "c/ISvcAsync.java",
    "package c; public interface ISvcAsync { void go(int a, AsyncCallback<String> cb); }",
);
// `entry` layer (@Controller) calling a `business` one (@Service).
const CLIENT: (&str, &str) = (
    "c/Page.java",
    "package c; @Controller public class Page {
  private ISvcAsync svc = ISvc.Util.getInstance();
  void run() { svc.go(1, null); }
}",
);
const SERVER: (&str, &str) = (
    "s/SvcImpl.java",
    "package s; import c.ISvc; @Service public class SvcImpl implements ISvc { public String go(int a) { return null; } }",
);

#[test]
fn single_root_produces_rpc_edges_and_matrix_groups() {
    let dir = project("single", &[SERVICE, ASYNC, CLIENT, SERVER]);
    let a = pipeline::analyze(&dir, &offline(), &|_| {}).expect("analyze");
    let s = a.rpc_stats.as_ref().expect("rpc stats");
    assert_eq!((s.calls, s.linked), (1, 1), "{s:?}");
    let rpc: Vec<_> = a
        .graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Rpc)
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(rpc, [("c.Page#run(0)", "s.SvcImpl#go(1)")]);
    assert_eq!(a.matrix.rpc_edges.len(), 1, "{:?}", a.matrix.rpc_edges);
    assert!(a.matrix.http_edges.is_empty());
    // Not an HTTP or call/uses edge.
    let json = serde_json::to_string(&a.matrix).expect("serialize");
    assert!(json.contains("\"rpc_edges\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_rpc_calls_leave_the_output_untouched() {
    let dir = project(
        "none",
        &[("a/A.java", "package a; class A { void f() {} }")],
    );
    let a = pipeline::analyze(&dir, &offline(), &|_| {}).expect("analyze");
    assert!(a.rpc_stats.is_none());
    let json = serde_json::to_string(&a.matrix).expect("serialize");
    assert!(!json.contains("rpc"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_roots_link_once_across_projects() {
    // Client + service in one root, the implementation in another.
    let front = project("front", &[SERVICE, ASYNC, CLIENT]);
    let back = project("back", &[SERVER]);
    let a = pipeline::analyze_many(&[front.clone(), back.clone()], &offline(), &|_| {})
        .expect("analyze");
    let s = a.rpc_stats.as_ref().expect("rpc stats");
    assert_eq!((s.calls, s.linked), (1, 1), "{s:?}");
    let rpc: Vec<_> = a
        .graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Rpc)
        .collect();
    assert_eq!(rpc.len(), 1, "{rpc:?}");
    assert!(rpc[0].from.ends_with("::c.Page#run(0)"), "{rpc:?}");
    assert!(rpc[0].to.ends_with("::s.SvcImpl#go(1)"), "{rpc:?}");
    assert_eq!(a.matrix.rpc_edges.len(), 1);
    let _ = std::fs::remove_dir_all(&front);
    let _ = std::fs::remove_dir_all(&back);
}
