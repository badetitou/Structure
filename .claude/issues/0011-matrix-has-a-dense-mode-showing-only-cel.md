---
id: 11
title: Matrix has a dense mode showing only cell colours in a packed grid
status: done
parent: null
branch: 11-dense-matrix
remote: null
created: 2026-11-06
---
## Context

The matrix view (`matrixView` in app/ui/src/main.ts) spaces cells on a 140px
pitch, sizes squares by symbol count, writes `layer/domain (size)` under each
square and draws inter-cell edges. On large projects (many domains/layers) it
gets huge and unreadable. The user wants a "dense" mode to see the whole
structure at a glance: only the colouring matters.

## Acceptance criteria
- [x] A toolbar toggle switches between the current view and a dense view; the
      choice is kept when re-rendering (and per viewer via localStorage, guarded).
- [x] Dense view: one uniform square per `layer × domain` slot, packed edge to
      edge (no gaps beyond a ≤1px separator), empty slots drawn as a faint tile.
- [x] No label under cells and no edges drawn. Layer/domain headers stay but are
      compact (small font, domain names rotated) so the grid fits the viewport.
- [x] Colouring is unchanged: hover reach (forward/backward/both), impact
      (changed/impacted distance/dimmed) and selection outline all still work;
      click still fills the side panel.
- [x] A native tooltip (`<title>`) on each tile gives `layer/domain (size)`.
- [x] Layout maths (tile position / viewBox size per mode) is a pure function
      with vitest tests; `npm test` and `npm run build` pass.

## Out of scope
- Encoding size in dense mode (colour scale by size), zoom/pan, reordering
  rows/columns, any Rust/API change.
