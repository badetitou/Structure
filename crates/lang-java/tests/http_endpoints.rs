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
        assert!(!s.http.is_empty(), "{} has no endpoint", s.id);
        n += s.http.len();
    }
    assert!(methods > 0, "filter matched no controller method");
    assert_eq!(n, 19);
    let ep = |id: &str| -> Vec<(HttpVerb, String)> {
        g.symbols
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .http
            .iter()
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
