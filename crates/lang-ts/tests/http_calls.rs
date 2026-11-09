use std::path::PathBuf;

use structure_ir::ir::HttpVerb::{self, *};
use structure_lang_ts::{parse_project, parse_source};

fn calls(src: &str, sym: &str) -> Vec<(HttpVerb, String)> {
    let f = parse_source("a.ts", src, false).unwrap();
    let s = f.symbols.iter().find(|s| s.id.ends_with(sym)).unwrap();
    assert!(s.http.is_empty());
    s.http_calls
        .iter()
        .map(|e| (e.verb, e.path.clone()))
        .collect()
}

fn p(v: HttpVerb, s: &str) -> (HttpVerb, String) {
    (v, s.to_string())
}

#[test]
fn http_client_typed_param_with_generics() {
    let src = "class S { constructor(private client: HttpClient) {}\n\
      load() { return this.client.get<{ a: A }>('/user'); }\n\
      save() { this.client.put<void>('/user', {}); this.client.patch('/p', {}); this.client.post('/q', {}); this.client.delete('/d'); } }";
    assert_eq!(calls(src, "S.load"), [p(Get, "/user")]);
    assert_eq!(
        calls(src, "S.save"),
        [
            p(Put, "/user"),
            p(Patch, "/p"),
            p(Post, "/q"),
            p(Delete, "/d")
        ]
    );
}

#[test]
fn http_client_injected_and_field_typed() {
    let src = "class S { private h = inject(HttpClient); private c: HttpClient;\n\
      a() { return this.h.get('users/'); }\n b() { return this.c.get('/x'); } }";
    assert_eq!(calls(src, "S.a"), [p(Get, "/users")]);
    assert_eq!(calls(src, "S.b"), [p(Get, "/x")]);
}

#[test]
fn http_client_request() {
    let src = "class S { constructor(private http: HttpClient) {}\n\
      a() { return this.http.request('DELETE', '/r'); }\n\
      b() { return this.http.request(m, '/r'); } }";
    assert_eq!(calls(src, "S.a"), [p(Delete, "/r")]);
    assert_eq!(calls(src, "S.b"), [p(Any, "/r")]);
}

#[test]
fn fetch_with_and_without_method() {
    let src = "function a() { return fetch('/f'); }\n\
      const b = () => fetch('/g', { method: 'POST', body: '' });\n\
      async function c() { await fetch('/h', { headers: {} }); }";
    assert_eq!(calls(src, "#a"), [p(Get, "/f")]);
    assert_eq!(calls(src, "#b"), [p(Post, "/g")]);
    assert_eq!(calls(src, "#c"), [p(Get, "/h")]);
}

#[test]
fn axios_forms() {
    let src = "function a() { axios.get('/a'); axios.delete('/b'); }\n\
      function b() { axios({ method: 'put', url: '/c' }); axios('/d'); }";
    assert_eq!(calls(src, "#a"), [p(Get, "/a"), p(Delete, "/b")]);
    assert_eq!(calls(src, "#b"), [p(Put, "/c"), p(Get, "/d")]);
}

#[test]
fn template_and_concatenation() {
    let src = "class S { constructor(private http: HttpClient) {}\n\
      a(slug: string, o: O) { this.http.get(`/articles/${slug}/c/${o.id}/`); this.http.get(`/x/${f()}`); }\n\
      b(u: string) { return this.http.get('/profiles/' + u); }\n\
      c() { return this.http.get('/s?limit=1'); this.http.get('https://h.io//api/'); } }";
    assert_eq!(
        calls(src, "S.a"),
        [p(Get, "/articles/{slug}/c/{id}"), p(Get, "/x/{}")]
    );
    assert_eq!(calls(src, "S.b"), [p(Get, "/profiles/{u}")]);
    assert_eq!(
        calls(src, "S.c"),
        [p(Get, "/s"), p(Get, "https://h.io/api")]
    );
}

#[test]
fn non_literal_url_is_unknown() {
    let src = "class S { constructor(private http: HttpClient) {}\n\
      a(u: string) { this.http.get(u); this.http.post('/a' + (c ? '/b' : '')); this.http.get(); } }";
    assert_eq!(calls(src, "S.a"), [p(Get, "?"), p(Post, "?"), p(Get, "?")]);
}

#[test]
fn non_http_receivers_are_ignored() {
    let src = "class S { m = new Map<string, number>();\n\
      a() { this.m.get('k'); this.m.delete('k'); cache.get('x'); request('/r'); } }\n\
      function top() { fetch('/ok'); }\n\
      fetch('/top-level');";
    assert!(calls(src, "S.a").is_empty());
    assert_eq!(calls(src, "#top"), [p(Get, "/ok")]);
    let f = parse_source("a.ts", src, false).unwrap();
    assert_eq!(
        f.symbols
            .iter()
            .filter(|s| !s.http_calls.is_empty())
            .count(),
        1
    );
}

#[test]
fn angular_realworld_services() {
    let fixtures = std::env::var("STRUCTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let root = fixtures.join("angular-realworld");
    if !root.is_dir() {
        eprintln!("skipping: {} not found", root.display());
        return;
    }
    let g = parse_project(&root).unwrap();
    let all: Vec<(&str, HttpVerb, &str)> = g
        .symbols
        .iter()
        .flat_map(|s| {
            s.http_calls
                .iter()
                .map(move |c| (s.id.as_str(), c.verb, c.path.as_str()))
        })
        .collect();
    eprintln!("angular-realworld http calls: {all:?}");
    assert_eq!(all.len(), 18, "{all:?}");
    let n = |v: HttpVerb| all.iter().filter(|(_, vb, _)| *vb == v).count();
    assert_eq!((n(Get), n(Post), n(Put), n(Delete)), (6, 6, 2, 4));
    let has = |suffix: &str, v: HttpVerb, path: &str| {
        all.iter()
            .any(|(id, vb, pa)| id.ends_with(suffix) && *vb == v && *pa == path)
    };
    assert!(has("UserService.getCurrentUser", Get, "/user"));
    assert!(has("UserService.update", Put, "/user"));
    assert!(has("UserService.login", Post, "/users/login"));
    assert!(has("UserService.register", Post, "/users"));
    assert!(has("TagsService.getAll", Get, "/tags"));
    assert!(has("ProfileService.get", Get, "/profiles/{username}"));
    assert!(has(
        "CommentsService.getAll",
        Get,
        "/articles/{slug}/comments"
    ));
    assert!(has(
        "CommentsService.delete",
        Delete,
        "/articles/{slug}/comments/{commentId}"
    ));
    assert!(has("ArticlesService.delete", Delete, "/articles/{slug}"));
    assert!(has(
        "ArticlesService.favorite",
        Post,
        "/articles/{slug}/favorite"
    ));
    // `'/articles' + (cond ? '/feed' : '')` is not a plain concatenation.
    assert!(has("ArticlesService.query", Get, "?"));
    assert_eq!(all.iter().filter(|(_, _, p)| *p == "?").count(), 1);
}

#[test]
fn http_client_only_when_the_annotation_itself_is_http_client() {
    let src = "class S { cache: Map<string, HttpClient>; clients: HttpClient[]; h: HttpClient | null;\n\
      a() { this.cache.get('k'); this.clients.get('j'); }\n\
      b() { this.h.get('/ok'); } }";
    assert!(calls(src, "S.a").is_empty());
    assert_eq!(calls(src, "S.b"), [p(Get, "/ok")]);
}

#[test]
fn unreadable_method_is_any() {
    let src = "function a() { fetch('/1', init); fetch('/2', { ...o }); fetch('/3', { method }); fetch('/4', { method: m }); fetch('/5', {}); }\n\
      function b() { axios(config); axios('/6', { method: 'post' }); axios('/7', { ...o }); axios('/8'); }";
    assert_eq!(
        calls(src, "#a"),
        [
            p(Any, "/1"),
            p(Any, "/2"),
            p(Any, "/3"),
            p(Any, "/4"),
            p(Get, "/5")
        ]
    );
    assert_eq!(
        calls(src, "#b"),
        [p(Any, "?"), p(Post, "/6"), p(Any, "/7"), p(Get, "/8")]
    );
}
