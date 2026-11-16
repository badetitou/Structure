---
id: 61
title: HTTP edges link frontend calls to backend endpoints
status: done
parent: 57
branch: 58-http-edges
remote: 49
created: 2026-11-09
---
## Context

Needs #58 (merged graph), #59 (endpoints), #60 (calls).

## Acceptance criteria
- [x] New `EdgeKind::Http` from the TS caller symbol to the Java handler method.
- [x] Matching: same verb; paths compared segment by segment, placeholders match
  any segment; a common prefix on one side (e.g. `/api`, a base URL) is tolerated
  by suffix matching; ambiguous matches choose the most specific one, ties produce no edge.
- [x] Matrix aggregates Http edges like other kinds (cell edges count by kind),
  so front `api_client` cells link to back `entry` cells.
- [x] Run metrics report calls found / linked / unmatched.
- [x] realworld pair: ≥ 80% of the Angular service calls linked; unmatched ones listed.

## Out of scope
- UI styling of Http edges beyond what the matrix already shows per kind.

## Brief (implementer)

Goal: with several roots, TS `Symbol.http_calls` are matched to Java `Symbol.http`
endpoints and become `EdgeKind::Http` edges caller → handler, shown in the matrix.
Read first: crates/ir/src/ir.rs (`EdgeKind`, `HttpVerb`, `HttpEndpoint`, `Symbol.http`
= served, `Symbol.http_calls` = made), crates/core/src/pipeline/multi.rs (`merge`, where
the merged graph is built before `matrix::build`), crates/classify/src/matrix.rs
(`build`: cell edges only count Call|Uses), crates/ir/src/model.rs (Matrix, EdgeGroup),
crates/scip/src/metrics.rs (`kind_name`), crates/cli/src/main.rs (what it prints).
Design:
- IR: add `EdgeKind::Http` (rank after the others; serde name `http`; `kind_name` "http").
- Matching: a pure fn (new module `crates/core/src/http_link.rs`, re-exported) over the
  merged Graph → `(Vec<Edge>, HttpLinkStats)`. Calls with path `?` are counted, never linked.
  Verb: equal, or either side `Any`. Paths split on `/`, empty segments dropped; a `{...}`
  segment on either side matches any one segment. The call may carry extra leading segments
  (base URL such as `/api`, or a `{apiUrl}` placeholder): match the endpoint against the
  call's trailing segments (suffix match), and the reverse (endpoint longer, e.g. a
  context-path) is NOT allowed. Score = number of literal (non-placeholder) segments
  matched equal. Pick the highest score; on a tie between different handlers → no edge,
  count as ambiguous. One edge per (caller symbol, handler symbol) pair, deduplicated.
- Run it in `merge` only (≥ 2 roots), so single-root output stays byte-identical.
- Matrix: add `http_edges: Vec<EdgeGroup>` (serde default, skip if empty), built like
  `edges` but from Http edges only, lifted to top-level cells the same way. `edges` is
  unchanged (Http excluded there, as today for non Call/Uses kinds).
- Stats: `HttpLinkStats { calls, linked, unknown_path, ambiguous, unmatched: Vec<String> }`
  (unmatched = "caller VERB path"), stored on `Analysis` and printed by the CLI as
  `http: <linked>/<calls> linked, <unknown> unknown path, <amb> ambiguous`, then the unmatched
  lines. Document `http_edges` and the Http kind in docs/api.md.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings;
cargo test --workspace with STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures;
`cargo run -p structure-cli -- analyze --no-scip $F/spring-boot-realworld-example-app $F/angular-realworld`.
Done: unit tests for matching (placeholders both sides, Any verb, suffix base URL,
endpoint-longer rejected, tie → ambiguous, `?` skipped, dedup); a fixture test on the realworld
pair asserting ≥ 15 of the 18 calls linked (≥ 80%) and exact handlers for a few (e.g.
UserService GET /user → CurrentUserApi#currentUser, comments DELETE → CommentsApi#deleteComment);
single-root JSON unchanged (existing test). Report the exact linked count and every unmatched call
with the reason. CLAUDE.md rule 6: if below 80%, keep improving or say exactly what blocks it.
Rules: no unwrap/expect outside tests; no new deps. Fixtures are already cloned; never clone or build them.
