---
id: 74
title: Matrix aligns front and back domains on shared rows, split by a double line
status: done
parent: null
branch: 74-matrix-align-front-back
remote: 62
created: 2026-11-09
---
## Context

A multi-project analysis (frontend + backend) stacks the projects' domains one after the
other along the domain axis, and both share the layer axis. A front `api_client` cell and the
back `entry` cell it calls (HTTP/RPC, #63/#69) end up far apart; nothing shows where the front
stops and the back starts. The backend tree also often stays one collapsed column because the
initial expansion budget (12 columns) is spent on the first project.

## Acceptance criteria
- [x] "Aligned" view (toolbar toggle, persisted, default on, shown only when the matrix has
  >= 2 projects and remote (http/rpc) edges): the layer axis is split into one segment per
  project (only the layers that project uses, in global order; shared layers like `model`
  appear once per segment), segments separated by a gap with a double line, in every mode
  (dense included), both orientations.
- [x] Domain axis: each row holds at most one visible domain column per project; columns linked by
  remote edges are paired (heaviest link first), so a front domain and the back domain it calls
  share a row and the link is straight. Unpaired columns get their own row. Rows follow the
  caller ("primary") project's column order; secondary-only rows go last.
- [x] Row label shows each member column (e.g. `orders ⇄ order`); a group member stays
  expandable by clicking its part. Bands are drawn for the primary project only.
- [x] Initial expansion gives each project its own budget, so the backend is expanded too.
- [x] Selection, hover, details panel, impact keep working (ids remapped through `cellMap`).
- [x] Toggle off = today's matrix exactly (existing tests unchanged).
- [x] vitest, svelte-check, build green; checked in the browser on `?sample=matrix-multi`.

## Out of scope
- Collapsing a secondary-project group (no bands for it) — follow-up if needed.
- Rust/backend changes.

## Brief (implementer)

Goal: in a multi-project matrix, put a front domain and the back domain it calls on the same row,
with per-project layer segments separated by a double line.
Read first: app/README.md (theme: colours only in app.css; SVG attrs via svgattr.ts);
app/ui/src/domaintree.ts (`initialExpanded`, `columns`, `bands`, `aggregate` → `ViewMatrix`, `cellMap`);
layout.ts (`layout`, `gridCore`, `flameCore`, `Layout.rows/columns`); lib/components/MatrixView.svelte
(cols → vm → lay → tiles/edges/bands); matrix/ColumnHeaders.svelte (layer labels are keyed by text:
must change, `model` will repeat); api.ts (`projects?`, `httpEdges`, `rpcEdges`); graph.ts (`visibleEdges`);
mode.ts + state.svelte.ts + Toolbar.svelte (how `transposed` / `showHttp` toggles are persisted);
docs/samples/matrix-multi.json.
Design:
- New pure module `aligned.ts` (+ tests): `align(vm, cols, layers, projects, remote edges) →
  { vm: ViewMatrix (cells/edges/httpEdges/rpcEdges re-keyed, cellMap composed), rows: {key, members: Column[]}[],
  layers: {key, project, layer}[], segments: number[] (layer index where each new segment starts), primary }`.
  Column project = prefix before the first `/` of its first domain, matched to `projects[].name`.
  Aligned layer key `${project}|${layer}`; aligned cell id `${layerKey}:${rowKey}`; keep `ViewCell.layer`
  = aligned layer key, `label` = member column label. Pairing: weights per (colA,colB) of different projects
  from shown remote edges; sort desc, ties by keys (deterministic); join when each project slot is free.
  Primary = project with the most outgoing remote weight; its segment comes first.
- layout.ts: optional `layerTexts` (labels) and `gapsBefore: number[]` (layer indices getting an extra
  gap: e.g. 8px dense, 24 normal, flame too); return `separators: number[]` (centre of each gap along the
  layer axis). Default arguments = current output.
- MatrixView: when aligned, initial expansion = `initialExpanded` per project subtree (own budget, keys
  re-prefixed `g:0…` → `g:<i>…`), run `align` after `aggregate`, feed its layers/rows to `layout`, filter
  `bands` to the primary subtree, draw the separators as two parallel lines (token `--segment-line`
  light+dark in app.css) across the whole domain axis.
- ColumnHeaders: row labels as `<tspan>` per member joined by ` ⇄ `; a group tspan calls `onexpand(col.key)`.
- Toggle "Align projects" next to Swap axes, `structure.matrix.aligned`, default on, visible only when
  available.
Verify: `cd app/ui && npm test && npm run check && npm run build` (npm ci if node_modules missing).
Unit tests: pairing (1-1, many-to-one keeps heaviest, >2 projects, no remote edges → each column its own row),
cellMap composition, segments with shared layers, layout gaps/separators, defaults unchanged, toggle persistence.
Browser check with `?sample=matrix-multi` (normal + dense, both orientations); stop any server you start.
Rules: no new deps; no colour literals outside app.css; one concern; commit on the branch when green
(no Claude attribution).

## Result

Done: `aligned.ts` (pairing, per-project layer segments, cellMap composition), `layout.ts`
(`layerTexts`, `gapsBefore`, `separators`), MatrixView/ColumnHeaders/Toolbar wiring, token
`--segment-line`, toggle `structure.matrix.aligned`. vitest 181 tests, svelte-check and build green.

Browser (`?sample=matrix-multi`, headless Chromium via playwright-core): normal and dense, both
orientations load without errors; double line drawn between the angular and spring segments;
rows such as `angular-realworld/article ⇄ spring-boot-realworld-example-app/article` pair front and
back. The sample has no `domain_tree`, so the per-project expansion budget and the primary-only
bands were covered by reasoning and unit tests only, not in the browser. Flame not checked visually.

Known limits: pairing uses all remote edges, whatever the HTTP/RPC link toggles; a row never merges
two existing groups; cell tooltips and the details panel show the plain layer name (`layerName`).
