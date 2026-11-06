---
id: 25
title: Matrix UI stays responsive with thousands of cells and symbols
status: done
parent: 20
branch: 39-svelte-matrix
remote: null
created: 2026-11-06
---
## Context

See #20. `main.ts:282-316` builds SVG per cell and rebuilds on mode toggle;
`main.ts:336-395` paint() touches every cell and edge on hover; `graph.ts` reach()
rebuilds adjacency per call and uses `queue.shift()`; `main.ts:399-425` renders
symbol lists without virtualization. Depends on #17/#18 (collapse, zoom/pan) landing.

## Acceptance criteria
- [ ] Adjacency built once per matrix; BFS O(V+E).
- [ ] Hover repaints only what changes; symbol lists virtualized or capped with "show more".
- [ ] vitest for reach(); manual check on vscode under ~100 ms per hover.

## Out of scope
- Canvas/WebGL rewrite.
