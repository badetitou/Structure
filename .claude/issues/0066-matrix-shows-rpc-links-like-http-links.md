---
id: 66
title: Matrix shows RPC links like HTTP links
status: in-progress
parent: 54
branch: 66-matrix-rpc-links
remote: null
created: 2026-11-09
---
## Context

#63 adds `EdgeKind::Rpc` and `Matrix.rpc_edges` (GWT RPC, client → server impl). The UI from #60
only draws `http_edges`.

## Acceptance criteria
- [ ] `rpc_edges` expanded and drawn like http edges, with its own token/dash so HTTP and RPC differ;
  same hover/selection; a toolbar toggle per kind (or one "Remote links" toggle with both — decide in the brief).
- [ ] Details panel section lists RPC calls/called-by like HTTP.
- [ ] A docs/samples sample with rpc_edges (no absolute paths); vitest, svelte-check, build green; checked in browser.

## Brief (implementer)

Goal: the app draws `Matrix.rpc_edges` (#63) like `http_edges` (#60), visibly different, with its own toggle and details section.
Read first: app/README.md (theme rules, HTTP links toggle sentence); app/ui/src/expand.ts, api.ts (`httpEdges`),
domaintree.ts (`aggregate` remap of httpEdges), graph.ts (`visibleEdges`, `httpLinks`), cellstate.ts,
lib/components/MatrixView.svelte, matrix/MatrixEdge.svelte, mode.ts (`structure.httpEdges`), state.svelte.ts (`showHttp`, `view`),
Toolbar.svelte, DetailsPanel.svelte, app.css (`--edge-http`); their tests; docs/api.md (rpc_edges).
Design: generalise rather than copy — a remote-edge kind `"http" | "rpc"`:
- expand `rpc_edges` → `matrix.rpcEdges: CellEdge[]` (empty when absent); `aggregate` remaps it too.
- drawn in the same edge layer; `MatrixEdge` gets `kind` instead of the `http` boolean: rpc = dotted (`2 3`) with token
  `--edge-rpc` (light + dark, distinct from `--edge-http`) and its own arrow marker.
- hover adjacency includes rpc edges while shown.
- toolbar: "RPC links" toggle next to "HTTP links", persisted under `structure.rpcEdges` (default on), shown only when the
  matrix has rpc edges. Refactor `httpLinks` into a kind-generic helper.
- DetailsPanel: an "RPC" section like "HTTP" (calls / called by, view cells, gated on its toggle).
- Sample: build a tiny synthetic GWT project in a scratch dir (2-3 client classes, one `IFooService extends RemoteService`
  + `IFooServiceAsync`, one `FooServiceImpl`, distinct packages so they land in different cells), run
  `cargo run -p structure-cli -- analyze --no-scip <dir> -o docs/samples/matrix-rpc.json`, then replace absolute paths in it.
  Do NOT generate a sample from e-sedit-rh or wegf (proprietary code).
Verify: `cd app/ui && npm test && npm run check && npm run build` (run `npm ci` first if node_modules is missing).
Unit tests: expand rpc_edges, aggregate remap of rpcEdges, adjacency with rpc shown/hidden, toggle persistence, generic links helper
for both kinds. HTTP behaviour unchanged (existing tests).
Rules: no new deps; no colour literals outside app.css.
