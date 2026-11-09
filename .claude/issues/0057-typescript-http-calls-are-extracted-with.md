---
id: 57
title: TypeScript HTTP calls are extracted with their URL
status: todo
parent: 54
branch: null
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
