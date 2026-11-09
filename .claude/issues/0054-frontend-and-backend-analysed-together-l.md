---
id: 54
title: Frontend and backend analysed together, linked by their HTTP API calls
status: todo
parent: null
branch: null
remote: null
created: 2026-11-09
---
## Context

Structure analyses one root in one language per run (`pipeline::analyze(root)`;
`detect_language` picks the first match, so a root with both `pom.xml` and
`package.json` is Java only). `Graph`/`Matrix` carry a single `language`.
Users want to analyse a frontend (TS) and its backend (Java) together and see
which frontend code calls which backend endpoint through the HTTP API.

Test pair: `angular-realworld` + `spring-boot-realworld-example-app`
(same RealWorld API, two separate repos).

## Acceptance criteria
- [ ] #55 Several roots → one merged graph and matrix (CLI `analyze a b`).
- [ ] #56 Java endpoints (Spring `@*Mapping`, JAX-RS `@Path`) extracted with verb + path.
- [ ] #57 TS HTTP calls (Angular `HttpClient`, `fetch`, `axios`) extracted with verb + URL template.
- [ ] #58 `EdgeKind::Http` edges from TS caller to Java handler, visible in the matrix.
- [ ] #59 Tauri app: pick several folders; `docs/api.md` updated.
- [ ] On the realworld pair, most `ArticlesService`/`UserService` calls link to their controller.

## Out of scope
- Monorepo auto-detection of sub-projects under one root (later).
- OpenAPI/Swagger specs as a link source; GraphQL, WebSockets, gRPC.
- Backend languages other than Java, frontend other than TS.
