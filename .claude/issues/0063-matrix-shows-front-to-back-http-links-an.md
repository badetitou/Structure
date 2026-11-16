---
id: 63
title: Matrix shows front-to-back HTTP links and can hide them
status: done
parent: 57
branch: 60-matrix-http-links
remote: 51
created: 2026-11-09
---
## Context

#61 adds `Matrix.http_edges` (same wire form as `edges`, grouped by source cell),
filled only for multi-project analyses. The UI (`app/ui`) ignores it.

## Acceptance criteria
- [x] `api.ts` expands `http_edges` like `edges`; `MatrixView` draws them with a distinct
  style (theme token in app.css, no colour literal), with hover/selection like other edges.
- [x] A toolbar toggle shows/hides HTTP links (persisted like "Dense"); hidden when the matrix has none.
- [x] The details panel lists a cell's HTTP links (caller cell → handler cell, count).
- [x] vitest + svelte-check green; a `docs/samples` multi-project sample with http_edges.

## Out of scope
- Symbol-level HTTP details (which call → which endpoint) beyond counts.

## Brief (implementer)

Goal: the matrix draws `Matrix.http_edges` (front api_client → back entry links from #61),
distinct from code edges, with a toggle to hide them and a list in the details panel.
Read first: app/README.md (layout, theme rules: colours only in app.css, SVG via svgattr.ts);
app/ui/src/expand.ts (`expandMatrix`: wire edges → CellEdge) and api.ts (`Matrix`, `http_edges?` type);
app/ui/src/lib/components/MatrixView.svelte (edges, `adjacency`), matrix/MatrixEdge.svelte,
graph.ts, cellstate.ts (hover/selection per edge); mode.ts + Toolbar.svelte (how the mode
is persisted in localStorage — mirror it for the toggle); DetailsPanel.svelte; docs/samples/matrix.json.
Design: expand `http_edges` into `matrix.httpEdges: CellEdge[]` (empty when absent). Draw them in the
same edge layer with an `http` flag → MatrixEdge gets a dashed stroke and its own token
(`--edge-http` in light and dark). Hover/selection: include http edges in the adjacency so hovering a
front cell highlights its back handlers. Toggle "HTTP links" in the Toolbar, persisted under
`structure.httpEdges` (default on), shown only when the matrix has http edges; in dense mode
edges are hidden anyway. DetailsPanel: for the selected cell, a "HTTP" section listing outgoing and
incoming http links (other cell id, count).
Sample: generate `docs/samples/matrix-multi.json` from
`cargo run -p structure-cli -- analyze --no-scip $F/spring-boot-realworld-example-app $F/angular-realworld -o …`
(F=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures). Wire it so the browser fallback can load it
(e.g. `?sample=matrix-multi`), only if that is a few lines; otherwise just add the file.
Verify: `cd app/ui && npm test && npm run check && npm run build`; unit tests for expand (http_edges),
the toggle persistence, and adjacency including http edges. If a browser preview works
(`npm run dev` serves samples), say so but don't leave servers running.
Rules: no new deps; no colour literals outside app.css; keep `edges` behaviour unchanged when http_edges is absent.
