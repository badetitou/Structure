//! GWT RPC across roots, and what the merge must not double count
//! (tree-sitter only).

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
        std::env::temp_dir().join(format!("structure_gwt_merge_{name}_{}", std::process::id()));
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
const SERVER: (&str, &str) = (
    "s/SvcImpl.java",
    "package s; import c.ISvc; @Service public class SvcImpl implements ISvc { public String go(int a) { return null; } }",
);

fn rpc_edges(a: &pipeline::Analysis) -> Vec<(&str, &str)> {
    a.graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Rpc)
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect()
}

#[test]
fn async_type_from_another_root_links_in_the_merge() {
    // The client only names `ISvcAsync` (not parsed in its root); service,
    // twin and implementation live in the other root.
    let front = project(
        "xfront",
        &[(
            "w/Page.java",
            "package w; import c.ISvcAsync; @Controller public class Page {
  private ISvcAsync svc;
  void run() { svc.go(1, null); }
}",
        )],
    );
    let back = project("xback", &[SERVICE, ASYNC, SERVER]);
    let a = pipeline::analyze_many(&[front.clone(), back.clone()], &offline(), &|_| {})
        .expect("analyze");
    let s = a.rpc_stats.as_ref().expect("rpc stats");
    assert_eq!(
        (s.calls, s.linked, s.unresolved_service),
        (1, 1, 0),
        "{s:?}"
    );
    let e = rpc_edges(&a);
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].0.ends_with("::w.Page#run(0)"), "{e:?}");
    assert!(e[0].1.ends_with("::s.SvcImpl#go(1)"), "{e:?}");
    let _ = std::fs::remove_dir_all(&front);
    let _ = std::fs::remove_dir_all(&back);
}

#[test]
fn per_root_rpc_edges_are_dropped_before_the_merge() {
    // Everything in the first root: its own analysis already links it.
    let client = (
        "c/Page.java",
        "package c; public class Page {
  private ISvcAsync svc = ISvc.Util.getInstance();
  void run() { svc.go(1, null); }
}",
    );
    let one = project("dup1", &[SERVICE, ASYNC, client, SERVER]);
    let two = project(
        "dup2",
        &[("a/A.java", "package a; class A { void f() {} }")],
    );
    let alone = pipeline::analyze(&one, &offline(), &|_| {}).expect("analyze");
    assert_eq!(alone.rpc_stats.as_ref().map(|s| s.linked), Some(1));
    assert_eq!(rpc_edges(&alone).len(), 1);
    let a =
        pipeline::analyze_many(&[one.clone(), two.clone()], &offline(), &|_| {}).expect("analyze");
    let s = a.rpc_stats.as_ref().expect("rpc stats");
    assert_eq!((s.calls, s.linked), (1, 1), "{s:?}");
    assert_eq!(rpc_edges(&a).len(), 1);
    let _ = std::fs::remove_dir_all(&one);
    let _ = std::fs::remove_dir_all(&two);
}

#[test]
fn non_service_util_is_not_rpc() {
    let dir = project(
        "bundle",
        &[(
            "c/Page.java",
            "package c; public class Page {
  void run() { ICorePhaseConstants.Util.getInstance().PHASE_CONSOLE(); }
}",
        )],
    );
    let a = pipeline::analyze(&dir, &offline(), &|_| {}).expect("analyze");
    let s = a.rpc_stats.as_ref().expect("a maybe call exists");
    assert_eq!(
        (s.calls, s.maybe_rpc, s.unresolved_service),
        (0, 1, 0),
        "{s:?}"
    );
    assert!(s.unmatched.is_empty());
    assert!(a.matrix.rpc_edges.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
