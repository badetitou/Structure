---
id: 59
title: Java HTTP endpoints are extracted from Spring and JAX-RS annotations
status: done
parent: 57
branch: 56-java-http-endpoints
remote: 47
created: 2026-11-09
---
## Context

lang-java keeps annotation simple names only (crates/lang-java/src/lib.rs
`annotations_of`), so `@GetMapping("/x")` loses its path.

## Acceptance criteria
- [x] Java methods expose HTTP endpoints `{verb, path}`: Spring
  `@GetMapping/@PostMapping/@PutMapping/@DeleteMapping/@PatchMapping`,
  `@RequestMapping(method=...)`; `value`/`path` attribute or positional value;
  arrays give several endpoints; class-level `@RequestMapping` prefix joined.
- [x] JAX-RS `@Path` (class + method) with `@GET/@POST/...`.
- [x] Path variables normalised (`{slug}` kept as a placeholder); constants
  not resolvable from literals are reported as unknown, not guessed.
- [x] Stored in the IR (e.g. `Symbol.http_endpoints`), serde default empty
  so existing JSON is unchanged when there are none.
- [x] Unit tests + realworld-java: every controller method has an endpoint.

## Out of scope
- `server.servlet.context-path` / properties files (note as a known limit).

## Brief (implementer)

Goal: Java methods carry the HTTP endpoints they serve, `{verb, path}`, in the IR.
Read first: crates/lang-java/src/lib.rs (`annotations_of` ~101-116, method
symbol creation ~194-296), crates/ir/src/ir.rs (Symbol). Look at
fixtures/spring-boot-realworld-example-app/src/main/java/**/api/*.java for real cases.
Design: in crates/ir add `pub struct HttpEndpoint { verb: HttpVerb, path: String }`
(`HttpVerb` = Get/Post/Put/Delete/Patch/Any, serde lowercase) and
`Symbol.http: Vec<HttpEndpoint>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
Name it neutrally: #60 will reuse the same type for TS *calls*.
In lang-java, read annotation arguments with tree-sitter: positional string,
`value =` / `path =`, string arrays → one endpoint each; `@RequestMapping(method = RequestMethod.X)`
(array too; absent → Any). Join the class-level `@RequestMapping`/`@Path` prefix with the method path
(normalise: single `/`, leading `/`, no trailing `/`). JAX-RS: `@Path` + `@GET/@POST/...`.
A non-literal value (a constant) gives path `?` — no guessing.
Keep `annotations` (names) as is: the classifier relies on it.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings;
cargo test --workspace with STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures.
Done: unit tests (Spring class prefix + method, arrays, RequestMethod, JAX-RS, constant → `?`);
a fixture test that every `@RestController` method in realworld-java has ≥ 1 endpoint; goldens unchanged.
Rules: no unwrap/expect outside tests; no new deps; don't touch pipeline/multi-root (#58 runs in parallel and also edits crates/ir — keep IR edits small and additive).
Fixtures are already cloned and indexed by the user — never clone or build them.

## Known limits
- `server.servlet.context-path` and properties files are not read.
- Class prefix comes only from the type's own annotation (not a superclass/interface).
