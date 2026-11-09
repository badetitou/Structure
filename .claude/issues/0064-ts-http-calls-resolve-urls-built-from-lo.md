---
id: 64
title: TS HTTP calls resolve URLs built from local variables
status: done
parent: 57
branch: 61-ts-url-local-vars
remote: null
created: 2026-11-09
---
## Context

Real test, wegf backend + frontend (`--no-scip`): `http: 1417/2043 linked, 1 unknown
path, 10 ambiguous` (69%). 544 of the 615 unmatched calls come from the OpenAPI-generated
clients (`frontend/generated/wegf-open-api-*/api/*.service.ts`):

    let localVarPath = `/habilitations/hasHabilitationNiveauApAe`;
    return this.httpClient.request<boolean>('get', `${this.configuration.basePath}${localVarPath}`, ...

`${...}${localVarPath}` becomes `/{basePath}{localVarPath}`, one segment, so it never matches.
Another 68 call `{apiComptaUrl}/Dette/...`, `/UcCollectivite/...` (the `casino` services): the
backend has no such endpoint, so they are correctly unmatched.

## Acceptance criteria
- [x] In a template/concatenated URL, an identifier substitution resolves to the string/template
  initialiser of a `const`/`let` in the same function (or a readonly class field), if that variable
  is never reassigned; resolution is one level deep, otherwise placeholder as today.
- [x] Adjacent placeholders with no `/` between them (`${a}${b}`) no longer fuse into one segment
  when `b` resolves to a path starting with `/`.
- [x] Unit tests for the OpenAPI generator shape (with `encodeParam` path params), reassignment → placeholder.
- [ ] wegf backend + frontend: linked ratio reported before/after, target >= 80% of calls whose
  endpoint exists in the backend (casino calls excluded and counted separately); angular-realworld
  test stays exact (18 calls).

## Out of scope
- Cross-function data flow, environment files, interceptors.

## Brief (implementer)

Goal: TS URL extraction follows a local variable holding the path, so OpenAPI-generated
clients (`${this.configuration.basePath}${localVarPath}`) produce real paths.
Read first: crates/lang-ts/src/http.rs (`raw_url`, placeholder, `normalize`), crates/lang-ts/src/lib.rs
(where calls are attached), crates/lang-ts/tests/http_calls.rs; crates/core/src/http_link.rs (matching:
suffix match, `{...}` = one segment, >= 1 literal segment). Real input, read-only:
/Users/benoit.verhaeghe/Documents/WeMagnus/wegf/frontend/generated/wegf-open-api-compta/api/habilitations.service.ts
(lines ~149-150) and another file with a path parameter (`encodeParam`).
Design: when a template substitution or `+` operand is an identifier, look in the enclosing function
body for `const|let <id> = <string|template|concat>` declared before the call and never assigned again
in that body (`id =`, `id +=`); if found, inline its URL form (recursively through raw_url, depth 1,
no further variable lookup), else placeholder as today. Also readonly/initialised class fields read as
`this.<id>` only if trivial; skip otherwise. Placeholders adjacent to a resolved `/…` stay separate
segments: `${base}${p}` with p=`/a/b` → `/{base}/a/b`.
Measure: `cargo build --release -p structure-cli` then
`./target/release/structure analyze --no-scip /Users/benoit.verhaeghe/Documents/WeMagnus/wegf/backend /Users/benoit.verhaeghe/Documents/WeMagnus/wegf/frontend -o <scratch>.json`
— before: `http: 1417/2043 linked, 1 unknown path, 10 ambiguous`. Report after, the unmatched grouped by
first segment, and the ratio excluding `{apiComptaUrl}` casino calls (endpoints not in that backend).
Target (CLAUDE.md rule 6): >= 80% of calls whose endpoint exists; if not reached, say exactly what blocks it.
Don't write anything in the wegf tree, don't build it, don't run its npm/maven.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings;
cargo test --workspace with STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures
(angular-realworld exact 18-call test and realworld 17/18 must stay). Unit tests: OpenAPI shape,
path param via encodeParam → `{}`, reassigned variable → placeholder, variable declared after the call → placeholder.
Rules: no unwrap/expect outside tests; no new deps.

## Result
- wegf backend+frontend: 1417 -> 1419/2043. The 542 OpenAPI calls now resolve (`/{basePath}/habilitations/...`) but their endpoints are on generated server interfaces under `target/` -> blocked by #68. Criterion 4 left open until #68.
- A variable captured by a closure (declared in an outer function) stays a placeholder (scope stops at the function boundary).
