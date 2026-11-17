use std::path::{Path, PathBuf};

use structure_ir::ir::{Graph, PortKey, PortRole, protocol};
use structure_lang_java::parse_project;

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn layout(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("structure_gwt_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let m = "src/main/java/";
    write(
        &dir,
        &format!("{m}c/remote/ISvc.java"),
        "package c.remote;
import com.google.gwt.user.client.rpc.RemoteService;
public interface ISvc extends RemoteService {
  String go(int a);
  class Util { public static ISvcAsync getInstance() { return GWT.create(ISvc.class); } }
}",
    );
    write(
        &dir,
        &format!("{m}c/remote/ISvcAsync.java"),
        "package c.remote;
public interface ISvcAsync { void go(int a, AsyncCallback<String> cb); }",
    );
    // A sub-service: extends ISvc, so it is a RemoteService too.
    write(
        &dir,
        &format!("{m}c/remote/ISub.java"),
        "package c.remote; public interface ISub extends ISvc { void more(); }",
    );
    write(
        &dir,
        &format!("{m}c/remote/ISubAsync.java"),
        "package c.remote; public interface ISubAsync extends ISvcAsync { void more(AsyncCallback<Void> cb); }",
    );
    // Not an RPC service: same Async naming, no RemoteService.
    write(
        &dir,
        &format!("{m}c/other/IFoo.java"),
        "package c.other; public interface IFoo { void run(); }",
    );
    write(
        &dir,
        &format!("{m}c/other/IFooAsync.java"),
        "package c.other; public interface IFooAsync { void run(Cb cb); }",
    );
    write(
        &dir,
        &format!("{m}s/SvcImpl.java"),
        "package s; import c.remote.ISvc;
public class SvcImpl extends RemoteServiceServlet implements ISvc { public String go(int a) { return null; } }",
    );
    write(
        &dir,
        &format!("{m}s/SvcImpl2.java"),
        "package s; import c.remote.*;
public class SvcImpl2 implements ISvc { public String go(int a) { return null; } }",
    );
    write(
        &dir,
        &format!("{m}c/ui/Page.java"),
        "package c.ui;
import c.remote.*;
import c.other.*;
public class Page {
  private ISvcAsync field = ISvc.Util.getInstance();
  private IFooAsync foo;
  void viaUtil() { ISvc.Util.getInstance().go(1, cb); }
  void viaField() { field.go(1, cb); }
  void viaThis() { this.field.go(2, cb); }
  void viaLocal() { ISvcAsync s = ISvc.Util.getInstance(); s.go(1, cb); }
  void viaVar() { var s = ISvc.Util.getInstance(); s.go(1, cb); }
  void viaCreate() { ISvcAsync s = GWT.create(ISvc.class); s.go(1, cb); }
  void viaCast() { ((ISvcAsync) GWT.create(ISvc.class)).go(1, cb); }
  void viaParam(ISvcAsync p) { p.go(1, cb); }
  void viaSub(ISubAsync p) { p.more(cb); }
  void shadowed() { IFooAsync field = foo; field.run(cb); }
  void notRpc() { foo.run(cb); field.toString(); }
  void inLambda() { run(() -> field.go(1, cb)); }
  Page() { field.go(0, cb); }
  void scoped1() { { IFooAsync field = foo; field.run(cb); } field.go(1, cb); }
  void scoped2() { run(() -> { IFooAsync field = foo; field.run(cb); }); field.go(1, cb); }
  void scoped3() { new Thread() { void r() { IFooAsync field = foo; } }; field.go(1, cb); }
  void scoped4() { for (IFooAsync field : foos) { field.run(cb); } field.go(1, cb); }
  void scoped5() { IFooAsync field = foo; { field.run(cb); } field.go(1, cb); }
}",
    );
    write(
        &dir,
        &format!("{m}c/ui/External.java"),
        "package c.ui;
public class External {
  void a() { com.x.IGone.Util.getInstance().f(cb); }
  void b() { IGone.Util.getInstance().f(cb); }
  void c() { ((IGoneAsync) GWT.create(IGone.class)).f(cb); }
  void d() { IGoneAsync typedOnly = null; typedOnly.f(cb); }
  void e() { Helper.Cache.get().f(cb); }
  void f() { ((Bundle) GWT.create(Bundle.class)).img(); }
  void i() { IGone.Util.getInstance().f(1, // comment
    cb); }
  void h() { Bundle.Util.get().img(); }
  void g() { GWT.create(Other.class).f(cb); }
}",
    );
    dir
}

fn maybe(g: &Graph, caller: &str) -> Vec<bool> {
    g.symbols
        .iter()
        .find(|s| s.id == caller)
        .unwrap_or_else(|| panic!("no {caller}"))
        .ports(protocol::GWT_RPC, PortRole::Requires)
        .filter_map(PortKey::as_service_method)
        .map(|c| c.maybe)
        .collect()
}

fn calls<'a>(g: &'a Graph, caller: &str) -> Vec<(&'a str, &'a str, u32)> {
    g.symbols
        .iter()
        .find(|s| s.id == caller)
        .unwrap_or_else(|| panic!("no {caller}"))
        .ports(protocol::GWT_RPC, PortRole::Requires)
        .filter_map(PortKey::as_service_method)
        .map(|c| (c.service.as_str(), c.method.as_str(), c.arity))
        .collect()
}

#[test]
fn rpc_call_shapes_are_extracted() {
    let dir = layout("shapes");
    let g = parse_project(&dir).unwrap();
    let svc = "c.remote.ISvc";
    for m in [
        "viaUtil",
        "viaField",
        "viaLocal",
        "viaVar",
        "viaCreate",
        "viaCast",
    ] {
        assert_eq!(
            calls(&g, &format!("c.ui.Page#{m}(0)")).first().copied(),
            Some((svc, "go", 2)),
            "{m}"
        );
    }
    assert_eq!(
        calls(&g, "c.ui.Page#viaParam(1)"),
        [(svc, "go", 2)],
        "param"
    );
    assert_eq!(calls(&g, "c.ui.Page#viaThis(0)"), [(svc, "go", 2)]);
    assert_eq!(
        calls(&g, "c.ui.Page#viaSub(1)"),
        [("c.remote.ISub", "more", 1)]
    );
    assert_eq!(calls(&g, "c.ui.Page#inLambda(0)"), [(svc, "go", 2)]);
    assert_eq!(calls(&g, "c.ui.Page#<init>(0)"), [(svc, "go", 2)]);
    // A comment between arguments is not an argument.

    // A local of another type shadows the field; `Object` methods are skipped.
    assert!(calls(&g, "c.ui.Page#shadowed(0)").is_empty());
    assert!(calls(&g, "c.ui.Page#notRpc(0)").is_empty());
    // IFoo has the Async naming but is no RemoteService.
    assert!(
        g.symbols
            .iter()
            .filter(|s| s.id.starts_with("c.ui.Page#"))
            .all(|s| s
                .ports(protocol::GWT_RPC, PortRole::Requires)
                .filter_map(PortKey::as_service_method)
                .all(|c| c.service != "c.other.IFoo"))
    );
    // The service and its Util never count as callers.
    assert!(
        g.symbols
            .iter()
            .filter(|s| s.id.starts_with("c.remote."))
            .all(|s| s
                .ports(protocol::GWT_RPC, PortRole::Requires)
                .filter_map(PortKey::as_service_method)
                .next()
                .is_none())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unresolved_service_keeps_its_simple_name() {
    let dir = layout("unresolved");
    let g = parse_project(&dir).unwrap();
    assert!(calls(&g, "c.ui.External#a(0)").is_empty());
    assert_eq!(calls(&g, "c.ui.External#b(0)"), [("IGone", "f", 1)]);
    assert_eq!(calls(&g, "c.ui.External#c(0)"), [("IGone", "f", 1)]);
    // Type only, or a nested type that is not `Util`: not enough to call it RPC.
    // `XAsync` typed but unknown here (imported from another root): kept as `X`.
    assert_eq!(calls(&g, "c.ui.External#d(0)"), [("IGone", "f", 1)]);
    assert!(maybe(&g, "c.ui.External#d(0)") == [false]);
    // No `IGoneAsync` type is parsed: only a maybe, the merged link decides.
    assert!(maybe(&g, "c.ui.External#b(0)") == [true]);
    assert!(maybe(&g, "c.ui.External#c(0)") == [true]);
    assert!(calls(&g, "c.ui.External#e(0)").is_empty());
    // ClientBundle / Constants created with GWT.create are not services.
    assert!(calls(&g, "c.ui.External#f(0)").is_empty());
    assert!(calls(&g, "c.ui.External#g(0)").is_empty());
    assert!(calls(&g, "c.ui.External#h(0)").is_empty());
    // A comment between arguments is not an argument.
    assert_eq!(calls(&g, "c.ui.External#i(0)"), [("IGone", "f", 2)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn locals_are_block_scoped() {
    let dir = layout("scoped");
    let g = parse_project(&dir).unwrap();
    let svc = "c.remote.ISvc";
    // The local of a closed block, lambda, anonymous class or loop no longer
    // hides the field.
    for m in ["scoped1", "scoped2", "scoped3", "scoped4"] {
        assert_eq!(
            calls(&g, &format!("c.ui.Page#{m}(0)")),
            [(svc, "go", 2)],
            "{m}"
        );
    }
    // Still in scope: shadows the field.
    assert!(calls(&g, "c.ui.Page#scoped5(0)").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
