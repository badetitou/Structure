use std::path::PathBuf;

use structure_ir::ir::{HttpVerb, SymbolKind};
use structure_lang_java::{parse_project, parse_source};

fn eps(src: &str, method: &str) -> Vec<(HttpVerb, String)> {
    let f = parse_source("A.java", src).unwrap();
    let m = f.symbols.iter().find(|s| s.name == method).unwrap();
    m.http.iter().map(|e| (e.verb, e.path.clone())).collect()
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
    for s in g.symbols.iter().filter(|s| {
        s.kind == SymbolKind::Method
            && s.annotations.iter().any(|a| a.ends_with("Mapping"))
            && s.container
                .as_deref()
                .is_some_and(|c| controllers.contains(&c))
    }) {
        assert!(!s.http.is_empty(), "{} has no endpoint", s.id);
        n += s.http.len();
        for e in &s.http {
            eprintln!("{:?} {}  <- {}", e.verb, e.path, s.id);
        }
    }
    eprintln!("{n} endpoints");
    assert!(n > 0);
}
