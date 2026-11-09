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
    assert_eq!(calls(src, "S.c"), [p(Get, "/s"), p(Get, "/api")]);
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
    // `ArticlesService.query` uses `'/articles' + (cond ? '/feed' : '')`: not a plain concatenation.
    let expected: Vec<(&str, HttpVerb, &str)> = vec![
        (
            "src/app/core/auth/services/user.service.ts#UserService.getCurrentUser",
            Get,
            "/user",
        ),
        (
            "src/app/core/auth/services/user.service.ts#UserService.login",
            Post,
            "/users/login",
        ),
        (
            "src/app/core/auth/services/user.service.ts#UserService.register",
            Post,
            "/users",
        ),
        (
            "src/app/core/auth/services/user.service.ts#UserService.update",
            Put,
            "/user",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.create",
            Post,
            "/articles",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.delete",
            Delete,
            "/articles/{slug}",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.favorite",
            Post,
            "/articles/{slug}/favorite",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.get",
            Get,
            "/articles/{slug}",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.query",
            Get,
            "?",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.unfavorite",
            Delete,
            "/articles/{slug}/favorite",
        ),
        (
            "src/app/features/article/services/articles.service.ts#ArticlesService.update",
            Put,
            "/articles/{slug}",
        ),
        (
            "src/app/features/article/services/comments.service.ts#CommentsService.add",
            Post,
            "/articles/{slug}/comments",
        ),
        (
            "src/app/features/article/services/comments.service.ts#CommentsService.delete",
            Delete,
            "/articles/{slug}/comments/{commentId}",
        ),
        (
            "src/app/features/article/services/comments.service.ts#CommentsService.getAll",
            Get,
            "/articles/{slug}/comments",
        ),
        (
            "src/app/features/article/services/tags.service.ts#TagsService.getAll",
            Get,
            "/tags",
        ),
        (
            "src/app/features/profile/services/profile.service.ts#ProfileService.follow",
            Post,
            "/profiles/{username}/follow",
        ),
        (
            "src/app/features/profile/services/profile.service.ts#ProfileService.get",
            Get,
            "/profiles/{username}",
        ),
        (
            "src/app/features/profile/services/profile.service.ts#ProfileService.unfollow",
            Delete,
            "/profiles/{username}/follow",
        ),
    ];
    let mut got = all.clone();
    got.sort_by_key(|(i, v, p)| (*i, *v as u8, *p));
    let mut want = expected;
    want.sort_by_key(|(i, v, p)| (*i, *v as u8, *p));
    assert_eq!(got, want);
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

#[test]
fn nested_class_does_not_wipe_outer_clients() {
    let src = "class O { constructor(private http: HttpClient) {}\n\
      a() { class I { m() {} } return new I(); }\n\
      b() { return this.http.get('/after'); } }";
    assert_eq!(calls(src, "O.b"), [p(Get, "/after")]);
}

#[test]
fn scheme_and_host_are_stripped() {
    let src = "function a() { fetch('http://localhost:8080/api/x'); fetch('https://h/x?q=1'); fetch('https://h'); }";
    assert_eq!(
        calls(src, "#a"),
        [p(Get, "/api/x"), p(Get, "/x"), p(Get, "/")]
    );
}

#[test]
fn calls_in_callbacks_belong_to_the_enclosing_method() {
    let src = "class S { constructor(private http: HttpClient) {}\n\
      a() { return o.pipe(switchMap(() => this.http.get('/x'))); }\n\
      b() { function inner() { return this.http.post('/y', {}); } return inner(); } }";
    let f = parse_source("a.ts", src, false).unwrap();
    let got: Vec<_> = f
        .symbols
        .iter()
        .filter(|s| !s.http_calls.is_empty())
        .map(|s| {
            (
                s.id.as_str(),
                s.http_calls[0].verb,
                s.http_calls[0].path.as_str(),
            )
        })
        .collect();
    assert_eq!(got, [("a.ts#S.a", Get, "/x"), ("a.ts#S.b", Post, "/y")]);
}

#[test]
fn url_from_local_variable() {
    let src = "class S { constructor(private httpClient: HttpClient) {}\n\
      a() { let localVarPath = `/habilitations/has`;\n\
        return this.httpClient.request<boolean>('get', `${this.configuration.basePath}${localVarPath}`, {}); }\n\
      b(id: number) { let localVarPath = `/users/${this.configuration.encodeParam({name: 'id', value: id})}/roles`;\n\
        return this.httpClient.request('delete', `${this.configuration.basePath}${localVarPath}`); }\n\
      c() { let p = '/a'; p = '/b'; return this.httpClient.get(`${p}/x`); }\n\
      d() { let p = '/a'; p += '/b'; return this.httpClient.get(`/api${p}`); }\n\
      e() { const r = this.httpClient.get(`/api${p}`); const p = '/late'; return r; }\n\
      f() { const q = '/a'; const p = `${q}/b`; return this.httpClient.get(`/api${p}`); } }";
    assert_eq!(calls(src, "S.a"), [p(Get, "/{basePath}/habilitations/has")]);
    assert_eq!(calls(src, "S.b"), [p(Delete, "/{basePath}/users/{}/roles")]);
    assert_eq!(calls(src, "S.c"), [p(Get, "/{p}/x")]);
    assert_eq!(calls(src, "S.d"), [p(Get, "/api{p}")]);
    assert_eq!(calls(src, "S.e"), [p(Get, "/api{p}")]);
    // Scope: a declarator must enclose the use, outside parameters and loop headers.
    let scoped = "class S { constructor(private http: HttpClient) {}\n\
      closure() { xs.forEach(() => { const u = '/a'; }); return this.http.get(`/api${u}`); }\n\
      sibling(a: boolean) { if (a) { const u = '/a'; } return this.http.get(`/api${u}`); }\n\
      param(u: string) { { const k = 1; } return this.http.get(`/api${u}`); }\n\
      shadow(u: string) { const u2 = 1; return this.http.get(`/api${u}`); }\n\
      forof(xs: string[]) { for (const u of xs) { this.http.get(`/api${u}`); } }\n\
      forin(o: any) { for (let u in o) { this.http.get(`/api${u}`); } }\n\
      arr() { let u = '/a'; [u] = xs; return this.http.get(`/api${u}`); }\n\
      obj() { let u = '/a'; ({u} = o); return this.http.get(`/api${u}`); }\n\
      obj2() { let u = '/a'; ({k: u} = o); return this.http.get(`/api${u}`); }\n\
      forre(xs: string[]) { let u = '/a'; for (u of xs) {} return this.http.get(`/api${u}`); }\n\
      forin2(o: any) { let u = '/a'; for (u in o) {} return this.http.get(`/api${u}`); }\n\
      nested() { const u = '/a'; if (x) { return this.http.get(`/api${u}`); } }\n\
      inner() { const u = '/a'; return xs.map(() => this.http.get(`/api${u}`)); } }";
    for m in [
        "closure", "sibling", "param", "forof", "forin", "arr", "obj", "obj2", "forre", "forin2",
    ] {
        assert_eq!(calls(scoped, &format!("S.{m}")), [p(Get, "/api{u}")], "{m}");
    }
    assert_eq!(calls(scoped, "S.nested"), [p(Get, "/api/a")]);
    // The function boundary stops the search: a use inside a callback stays a placeholder.
    assert_eq!(calls(scoped, "S.inner"), [p(Get, "/api{u}")]);
    // One level deep only: `q` inside `p` stays a placeholder.
    assert_eq!(calls(src, "S.f"), [p(Get, "/api{q}/b")]);
}
