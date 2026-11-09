---
id: 57
title: TypeScript HTTP calls are extracted with their URL
status: in-progress
parent: 54
branch: 57-ts-http-calls
remote: null
created: 2026-11-09
---
## Context

lang-ts does not capture call arguments; `this.http.get('/user')` produces
nothing in the graph (crates/lang-ts/src/lib.rs). Call edges come only from SCIP.

## Acceptance criteria
- [ ] TS functions/methods expose HTTP calls `{verb, url_template}` for:
  Angular `HttpClient` (`get/post/put/delete/patch/request`, receiver typed or
  injected as `HttpClient`), `fetch(url, {method})`, `axios.<verb>` / `axios(...)`.
- [ ] URL from a string literal, a template literal (`${x}` → placeholder) or
  `'a/' + x` concatenation; anything else is an unknown URL, kept with its verb.
- [ ] Stored in the IR next to #56's endpoints (shared type), serde default empty.
- [ ] Unit tests + angular-realworld: the calls in `core/**/services` are found.

## Out of scope
- Following a URL through variables, environment files or interceptors (a
  leading base-URL segment is handled by matching in #58).

## Brief (implementer)

Goal: TS functions/methods carry the HTTP calls they make, as `{verb, path}`, in the IR.
Read first: crates/ir/src/ir.rs (`HttpVerb`, `HttpEndpoint`, `Symbol.http` from #56),
crates/lang-ts/src/lib.rs (symbol creation, `collect_inject`, how member
symbols `<classId>.<member>` are built), crates/lang-java/tests/http_endpoints.rs (test style).
Real cases: fixtures/angular-realworld/src/app/**/services/*.service.ts, e.g.
`this.http.get<{ user: User }>('/user')`, `` this.http.delete<void>(`/articles/${slug}`) ``,
`this.http.get<...>('/profiles/' + username)`, and a multi-line `get<...>(\n ...)` in articles.service.ts.
IR design: `Symbol.http` means *served*. Add a separate
`Symbol.http_calls: Vec<HttpEndpoint>` (same serde attrs: default + skip if empty;
doc: "HTTP requests made by this symbol"), reusing `HttpEndpoint`/`HttpVerb`.
Don't change `http` or Java.
Extraction (tree-sitter only, no SCIP): inside a function or method body, for a call
expression whose callee is:
- `<recv>.get/post/put/delete/patch(url, ...)`, where `<recv>` is a member/identifier whose
  declared or injected type is `HttpClient`: a constructor param / field typed `HttpClient`,
  or `inject(HttpClient)`. A receiver named `http` with no known type is also accepted;
- `<recv>.request('VERB', url)`;
- `fetch(url, { method: 'X' })` (no method → Get);
- `axios.<verb>(url)` / `axios({ method, url })` / `axios(url)` (Get).
URL: string literal; template literal, with `${expr}` → `{expr}` when expr is an identifier
or member (last segment), else `{}`; `'a/' + x` → `a/{x}`. Normalise as Java
(leading `/`, no trailing `/`, single `/`); keep any scheme/host part as is; drop a
`?query`. Anything else → path `?`, verb kept.
The calls go on the innermost enclosing named function/method symbol; skip
calls in top-level code with no enclosing symbol.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings;
cargo test --workspace with STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures.
Done: unit tests per form (HttpClient typed + inject + generic type args, request, fetch with
and without method, axios forms, template, concatenation, non-literal → `?`, a non-HttpClient
`.get` on a Map → nothing); an angular-realworld fixture test that asserts exact
verb+path for ≥ 5 known calls (e.g. Get /user on UserService, Delete
/articles/{slug}/comments/{commentId}) and a total that cannot pass vacuously; goldens unchanged.
Rules: no unwrap/expect outside tests; no new deps. Fixtures are already cloned by the user; never clone or build them.
