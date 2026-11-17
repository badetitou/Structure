use std::path::PathBuf;

use structure_ir::ir::{HttpVerb, PortKey, PortRole, SymbolKind, protocol};
use structure_lang_java::{parse_project, parse_source};

fn eps(src: &str, method: &str) -> Vec<(HttpVerb, String)> {
    let f = parse_source("A.java", src).unwrap();
    let m = f.symbols.iter().find(|s| s.name == method).unwrap();
    m.ports(protocol::HTTP, PortRole::Provides)
        .filter_map(PortKey::as_endpoint)
        .map(|e| (e.verb, e.path.clone()))
        .collect()
}

#[test]
fn spring_prefix_and_method() {
    let src = r#"@RestController @RequestMapping("/articles/{slug}")
class A { @GetMapping("comments/") void a() {} @PostMapping void b() {} void c() {} }"#;
    assert_eq!(
        eps(src, "a"),
        [(HttpVerb::Get, "/articles/{slug}/comments".into())]
    );
    assert_eq!(eps(src, "b"), [(HttpVerb::Post, "/articles/{slug}".into())]);
    assert!(eps(src, "c").is_empty());
}

#[test]
fn spring_arrays_and_request_method() {
    let src = r#"class A {
  @GetMapping(path = {"/x", "y"}) void a() {}
  @RequestMapping(value = "/z", method = {RequestMethod.PUT, DELETE}) void b() {}
  @RequestMapping("/any") void c() {}
}"#;
    assert_eq!(
        eps(src, "a"),
        [(HttpVerb::Get, "/x".into()), (HttpVerb::Get, "/y".into())]
    );
    assert_eq!(
        eps(src, "b"),
        [
            (HttpVerb::Put, "/z".into()),
            (HttpVerb::Delete, "/z".into())
        ]
    );
    assert_eq!(eps(src, "c"), [(HttpVerb::Any, "/any".into())]);
}

#[test]
fn unknown_request_method_gives_no_endpoint() {
    let src = r#"class A {
  @RequestMapping(value = "/h", method = RequestMethod.HEAD) void a() {}
  @RequestMapping(value = "/c", method = Consts.M) void b() {}
  @RequestMapping(value = "/m", method = {RequestMethod.GET, RequestMethod.OPTIONS}) void c() {}
}"#;
    assert!(eps(src, "a").is_empty());
    assert!(eps(src, "b").is_empty());
    assert_eq!(eps(src, "c"), [(HttpVerb::Get, "/m".into())]);
}

#[test]
fn jaxrs_path_and_verb() {
    let src = r#"@Path("/api") class A {
  @GET @Path("/items/{id}") void a() {}
  @POST void b() {}
  @Path("/nope") void c() {}
}"#;
    assert_eq!(eps(src, "a"), [(HttpVerb::Get, "/api/items/{id}".into())]);
    assert_eq!(eps(src, "b"), [(HttpVerb::Post, "/api".into())]);
    assert!(eps(src, "c").is_empty());
}

#[test]
fn http_clients_serve_no_endpoint() {
    let src = r#"@FeignClient(name = "x") @RequestMapping("/api")
interface F { @GetMapping("/a") void a(); }
@HttpExchange("/b") interface H { @GetMapping("/c") void c(); }"#;
    assert!(eps(src, "a").is_empty());
    assert!(eps(src, "c").is_empty());
}

#[test]
fn constant_path_is_unknown() {
    let src = r#"class A { @GetMapping(Consts.X) void a() {} @GetMapping("a" + "b") void b() {} }"#;
    assert_eq!(eps(src, "a"), [(HttpVerb::Get, "?".into())]);
    assert_eq!(eps(src, "b"), [(HttpVerb::Get, "?".into())]);
}

#[test]
fn realworld_controller_methods_have_endpoints() {
    let base = std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let root = base.join("spring-boot-realworld-example-app");
    if !root.exists() {
        eprintln!("skipping: {} not found", root.display());
        return;
    }
    let g = parse_project(&root).unwrap();
    let controllers: Vec<&str> = g
        .symbols
        .iter()
        .filter(|s| s.annotations.iter().any(|a| a == "RestController"))
        .map(|s| s.id.as_str())
        .collect();
    assert!(!controllers.is_empty());
    let mut n = 0;
    let mut methods = 0;
    for s in g.symbols.iter().filter(|s| {
        s.kind == SymbolKind::Method
            && s.annotations.iter().any(|a| a.ends_with("Mapping"))
            && s.container
                .as_deref()
                .is_some_and(|c| controllers.contains(&c))
    }) {
        methods += 1;
        assert!(
            s.ports(protocol::HTTP, PortRole::Provides)
                .filter_map(PortKey::as_endpoint)
                .next()
                .is_some(),
            "{} has no endpoint",
            s.id
        );
        n += s
            .ports(protocol::HTTP, PortRole::Provides)
            .filter_map(PortKey::as_endpoint)
            .count();
    }
    assert!(methods > 0, "filter matched no controller method");
    assert_eq!(n, 19);
    let ep = |id: &str| -> Vec<(HttpVerb, String)> {
        g.symbols
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .ports(protocol::HTTP, PortRole::Provides)
            .filter_map(PortKey::as_endpoint)
            .map(|e| (e.verb, e.path.clone()))
            .collect()
    };
    assert_eq!(
        ep("io.spring.api.ArticleApi#article(2)"),
        [(HttpVerb::Get, "/articles/{slug}".into())]
    );
    assert_eq!(
        ep("io.spring.api.CommentsApi#deleteComment(3)"),
        [(HttpVerb::Delete, "/articles/{slug}/comments/{id}".into())]
    );
    assert_eq!(
        ep("io.spring.api.UsersApi#userLogin(1)"),
        [(HttpVerb::Post, "/users/login".into())]
    );
    assert_eq!(
        ep("io.spring.api.ArticlesApi#getFeed(3)"),
        [(HttpVerb::Get, "/articles/feed".into())]
    );
    assert_eq!(
        ep("io.spring.api.CurrentUserApi#updateProfile(3)"),
        [(HttpVerb::Put, "/user".into())]
    );
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("structure_http_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn write(dir: &std::path::Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn http_of(g: &structure_ir::ir::Graph, id: &str) -> Vec<(HttpVerb, String)> {
    g.symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .ports(protocol::HTTP, PortRole::Provides)
        .filter_map(PortKey::as_endpoint)
        .map(|e| (e.verb, e.path.clone()))
        .collect()
}

const API: &str = r#"package p;
@RequestMapping("/base")
interface Api {
  @GetMapping("/x/{id}") void get(String id);
  @PostMapping("/y") void post();
}"#;

#[test]
fn interface_endpoints_inherited_with_prefix() {
    let d = tmp("inherit");
    write(&d, "src/main/java/p/Api.java", API);
    write(
        &d,
        "src/main/java/p/Ctl.java",
        r#"package p; class Ctl implements Api { @Override public void get(String id) {}
           @PostMapping("/own") public void post() {} }"#,
    );
    write(
        &d,
        "src/main/java/p/Sub.java",
        r#"package p; @RequestMapping("/sub") class Sub implements Api { public void get(String id) {} }"#,
    );
    let g = parse_project(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(
        http_of(&g, "p.Ctl#get(1)"),
        [(HttpVerb::Get, "/base/x/{id}".into())]
    );
    // Own mapping wins; the interface method keeps its endpoint.
    assert_eq!(
        http_of(&g, "p.Ctl#post(0)"),
        [(HttpVerb::Post, "/base/own".into())]
    );
    assert_eq!(
        http_of(&g, "p.Api#post(0)"),
        [(HttpVerb::Post, "/base/y".into())]
    );
    // Spring: the class-level mapping of the implementation replaces the interface's.
    assert_eq!(
        http_of(&g, "p.Sub#get(1)"),
        [(HttpVerb::Get, "/sub/x/{id}".into())]
    );
}

#[test]
fn generated_interface_is_used_but_not_in_graph() {
    let d = tmp("gen");
    let gen_api = API
        .replace("package p;", "package gen;")
        .replace("Api", "HabApi");
    write(
        &d,
        "target/generated-sources/openapi/src/main/java/gen/HabApi.java",
        &gen_api,
    );
    write(
        &d,
        "build/generated/x/g/OtherApi.java",
        r#"package g; interface OtherApi { @GetMapping("/o") void o(); }"#,
    );
    write(
        &d,
        "src/main/java/p/Ctl.java",
        "package p; import gen.HabApi; class Ctl implements HabApi { public void get(String id) {} }",
    );
    write(
        &d,
        "src/main/java/p/Ctl2.java",
        "package p; class Ctl2 implements OtherApi { public void o() {} }",
    );
    write(
        &d,
        "src/main/java/p/Lost.java",
        "package p; class Lost implements MissingApi {}",
    );
    let mut warnings = Vec::new();
    let g = structure_lang_java::parse_project_with_warnings(&d, &mut warnings).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(
        http_of(&g, "p.Ctl#get(1)"),
        [(HttpVerb::Get, "/base/x/{id}".into())]
    );
    assert_eq!(http_of(&g, "p.Ctl2#o(0)"), [(HttpVerb::Get, "/o".into())]);
    assert!(
        g.symbols
            .iter()
            .all(|s| !s.id.starts_with("gen.") && !s.id.starts_with("g."))
    );
    assert!(g.edges.iter().all(|e| !e.to.starts_with("gen.")));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("1 class"), "{warnings:?}");
}

#[test]
fn feign_interface_and_fallback_serve_nothing() {
    let d = tmp("feign");
    write(
        &d,
        "src/main/java/p/FooClient.java",
        r#"package p; @FeignClient(name = "x") @RequestMapping("/f")
        interface FooClient { @GetMapping("/a") void a(); }"#,
    );
    write(
        &d,
        "src/main/java/p/FooClientFallback.java",
        "package p; class FooClientFallback implements FooClient { public void a() {} }",
    );
    let g = parse_project(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert!(http_of(&g, "p.FooClient#a(0)").is_empty());
    assert!(http_of(&g, "p.FooClientFallback#a(0)").is_empty());
}

#[test]
fn transitive_and_abstract_superclass_inheritance() {
    let d = tmp("trans");
    write(
        &d,
        "src/main/java/p/A.java",
        r#"package p; @RequestMapping("/a") interface A { @GetMapping("/m") void m(); }"#,
    );
    write(
        &d,
        "src/main/java/p/B.java",
        "package p; abstract class B implements A { }",
    );
    write(
        &d,
        "src/main/java/p/C.java",
        "package p; class C extends B { public void m() {} }",
    );
    write(
        &d,
        "src/main/java/p/Base.java",
        r#"package p; abstract class Base { @PutMapping("/z") abstract void z(); }"#,
    );
    write(
        &d,
        "src/main/java/p/D.java",
        r#"package p; @RequestMapping("/d") class D extends Base { void z() {} }"#,
    );
    let g = parse_project(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(http_of(&g, "p.C#m(0)"), [(HttpVerb::Get, "/a/m".into())]);
    assert_eq!(http_of(&g, "p.D#z(0)"), [(HttpVerb::Put, "/d/z".into())]);
}

#[test]
fn same_simple_name_generated_apis_resolve_by_import() {
    let d = tmp("twin");
    let g1 = "target/generated-sources/openapi/src/main/java/one/HabApi.java";
    let g2 = "target/generated-sources/openapi/src/main/java/two/HabApi.java";
    write(
        &d,
        g1,
        r#"package one; interface HabApi { @GetMapping("/one") void h(); }"#,
    );
    write(
        &d,
        g2,
        r#"package two; interface HabApi { @GetMapping("/two") void h(); }"#,
    );
    write(
        &d,
        "src/main/java/p/Ctl.java",
        "package p; import two.HabApi; class Ctl implements HabApi { public void h() {} }",
    );
    let g = parse_project(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(http_of(&g, "p.Ctl#h(0)"), [(HttpVerb::Get, "/two".into())]);
}

#[test]
fn no_generated_dir_leaves_methods_without_endpoints() {
    let d = tmp("nogen");
    write(
        &d,
        "src/main/java/p/Ctl.java",
        "package p; class Ctl implements HabApi { public void get(String id) {} }",
    );
    let mut warnings = Vec::new();
    let g = structure_lang_java::parse_project_with_warnings(&d, &mut warnings).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert!(http_of(&g, "p.Ctl#get(1)").is_empty());
    assert_eq!(warnings.len(), 1);
}
