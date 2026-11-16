---
id: 66
title: GWT RPC calls link the client to the server implementation
status: done
parent: 57
branch: 63-gwt-rpc
remote: 54
created: 2026-11-09
---
## Context

Requested by the user after the real test on e-sedit-rh (GWT): `analyze BLGRHGwt BLGRHServer`
gives `http: 0/0` — GWT talks RPC, not URLs. Shape in e-sedit-rh/BLGRHGwt (19 services):
- client `fr.bl.client.grh.ab.remote.IGwtServiceAb extends RemoteService`, its
  `IGwtServiceAbAsync` (same methods + `AsyncCallback` last param), obtained with
  `IGwtServiceAb.Util.getInstance()` / `GWT.create(IGwtServiceAb.class)`;
- server `fr.bl.server.grh.ab.GwtServiceAb extends RemoteServiceServlet implements IGwtServiceAb`.
Client and server live in the same project here, so this must work for one root too,
not only across roots like HTTP (#61).

## Acceptance criteria
- [x] A call to a method `m` of an `XAsync` interface whose sync twin `X extends RemoteService`
  gives an RPC edge from the caller to `Impl#m` for each class implementing `X` (arity = async arity - 1).
- [x] (analysis side; drawing in #69) New `EdgeKind::Rpc` (or reuse Http with a protocol tag — decide in the brief), shown in the
  matrix like http_edges (#63: drawn, toggle, details), in single- and multi-root analyses.
- [x] Works without SCIP (receiver typed `XAsync` field/local, or `X.Util.getInstance()` / `GWT.create(X.class)`),
  and with SCIP when available.
- [x] e-sedit-rh BLGRHGwt: linked/total RPC calls reported, target >= 80%, unmatched listed.
- [x] Unit tests on a minimal GWT layout.

## Out of scope (later RPC kinds)
- gRPC (generated stubs ↔ `*ImplBase`), Java RMI, JSON-RPC, Feign/HttpExchange clients as callers
  (#59 already skips them as servers).

UI drawing moved to #69; this issue is the analysis side (IR, extraction, linking, matrix field, CLI).

## Brief (implementer)

Goal: GWT RPC calls become `EdgeKind::Rpc` edges from the client caller method to the server
implementation method, in single- and multi-root analyses, without SCIP.
Read first: crates/ir/src/ir.rs (EdgeKind incl. Http rank, Symbol.http/http_calls), crates/lang-java/src/lib.rs
(supertype resolution `index.resolve`, how #59/#68 fill per-method data, method ids `<type>#<name>(<arity>)`,
Feign guard), crates/core/src/http_link.rs (shape to mirror) and pipeline/mod.rs + pipeline/multi.rs
(where link_http runs, single-root output byte-identical rule), crates/classify/src/matrix.rs (http_edges grouping),
crates/scip/src/metrics.rs kind names, crates/cli/src/main.rs (http: line).
Real shapes, read-only: /Users/benoit.verhaeghe/Documents/Sedit/e-sedit-rh/BLGRHGwt/src —
`fr/bl/client/grh/ab/remote/IGwtServiceAb.java` (`interface IGwtServiceAb extends RemoteService` with nested
`Util.getInstance()` returning `IGwtServiceAbAsync`), `IGwtServiceAbAsync.java`, server
`fr/bl/server/grh/ab/GwtServiceAb.java` (`extends RemoteServiceServlet implements IGwtServiceAb`), client call
`fr/bl/client/grh/ab/rtt/InjectionDiminutionRTTCommand.java:40` (field `IGwtServiceAbAsync serviceAb = IGwtServiceAb.Util.getInstance()`).
Call shapes by frequency: `IGwtServiceX.Util.getInstance().m(..)` (~4000), fields/locals typed `XAsync`
(`serviceCoeur.m(..)`), `GWT.create(X.class)` cast to XAsync.
Design:
- IR: `EdgeKind::Rpc` (rank after Http, serde/kind name `rpc`). `Symbol.rpc_calls: Vec<RpcCall{service: String /* FQN of
  the sync interface X, or simple name if unresolved */, method: String, arity: u32}>` (serde default, skip if empty).
- lang-java (tree-sitter): a sync service = interface whose supertypes include `RemoteService` (transitively within
  parsed code). Its async twin = `XAsync` in the same package. In a method body, a `method_invocation` whose receiver is
  (a) `X.Util.getInstance()` / any static call on `X.<Nested>` returning `XAsync`, (b) `GWT.create(X.class)` (possibly cast),
  (c) an identifier/`this.f` whose declared type (field, local, param) is `XAsync`, records an RpcCall on the enclosing
  method with arity = argument count. Skip calls inside the service/async interfaces themselves.
- Linking (new `crates/core/src/rpc_link.rs`, re-exported, pure fn over Graph → (edges, RpcLinkStats{calls, linked,
  unresolved_service, no_impl, unmatched: Vec<String>})): for each RpcCall, implementations = classes implementing X
  (transitively, parsed code); target = `Impl#method(arity-1)` (the AsyncCallback parameter is dropped); fall back to
  `arity` if no such method (some services return a Request). Several implementations → one edge each. Dedup per
  (caller, target).
- Run it at the end of single-root analysis AND on the merged graph in multi-root (do not double-count: per-root graphs
  feeding `merge` must not keep their own Rpc edges, or merge re-links from rpc_calls). Single-root output must stay
  byte-identical when there are no RpcCalls (all goldens).
- Matrix: `rpc_edges: Vec<EdgeGroup>` (serde default, skip if empty), built like `http_edges`. `edges` unchanged.
- Stats on Analysis + CLI line `rpc: <linked>/<calls> linked, <n> no implementation, <m> unresolved service`, then up to
  20 unmatched lines and "... and K more". docs/api.md + CLAUDE.md map (rpc_link).
Measure (read-only, never build or write there): `cargo build --release -p structure-cli`;
`./target/release/structure analyze --no-scip /Users/benoit.verhaeghe/Documents/Sedit/e-sedit-rh/BLGRHGwt -o <scratch>`
and the pair `.../BLGRHGwt .../BLGRHServer`. Report the rpc: line, wall time, unmatched grouped by service, and how many
RpcCalls target each of the 19 services. Target (CLAUDE.md rule 6): >= 80% linked; else say exactly what blocks it.
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace with
STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures (goldens and http stats unchanged).
Unit tests on a minimal GWT layout: each call shape, AsyncCallback arity drop, several impls, unresolved service, field
shadowed by a local of another type, single root produces rpc_edges.
Rules: no unwrap/expect outside tests; no new deps.

## Result
- e-sedit-rh BLGRHGwt `--no-scip`: `rpc: 5393/5393 linked` (0.8 s); 40 calls on services absent from the repo
  (`IGwtServiceSedit`, `IGwtServiceCore`, ... and `ICorePhaseConstants`) reported as "not counted" (`maybe_rpc`).
  Including them: 5393/5433 = 99.3%. 416 rpc_edges groups over 731 cells. Pair with BLGRHServer: identical.
- Known limits: same-simple-name implementation across roots has no package/import preference (Symbol has no
  imports); varargs arity; calls in field initialisers/static blocks not collected.
