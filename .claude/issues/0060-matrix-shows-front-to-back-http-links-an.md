---
id: 60
title: Matrix shows front-to-back HTTP links and can hide them
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

#58 adds `Matrix.http_edges` (same wire form as `edges`, grouped by source cell),
filled only for multi-project analyses. The UI (`app/ui`) ignores it.

## Acceptance criteria
- [ ] `api.ts` expands `http_edges` like `edges`; `MatrixView` draws them with a distinct
  style (theme token in app.css, no colour literal), with hover/selection like other edges.
- [ ] A toolbar toggle shows/hides HTTP links (persisted like "Dense"); hidden when the matrix has none.
- [ ] The details panel lists a cell's HTTP links (caller cell → handler cell, count).
- [ ] vitest + svelte-check green; a `docs/samples` multi-project sample with http_edges.

## Out of scope
- Symbol-level HTTP details (which call → which endpoint) beyond counts.
