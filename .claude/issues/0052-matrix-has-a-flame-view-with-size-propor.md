---
id: 52
title: Matrix has a flame view with size-proportional, deep domain columns
status: done
parent: null
branch: claude/matrix-view-depth-horizontal-79fbf8
remote: 40
created: 2026-11-09
---
## Context

Merged in 9c7de7d.

The matrix caps the default domain expansion at 12 columns and gives every
column the same pitch, so the domain hierarchy (#16) stays shallow and the
diagram stays square. The user wants depth that lets it grow horizontally,
like a flame graph / icicle: the domain tree stacked over several levels,
each block as wide as its symbol count.

## Acceptance criteria
- [x] A third matrix mode `flame` next to normal and dense (toolbar, persisted).
- [x] In flame mode the domain tree starts expanded much deeper (no 12-column cap).
- [x] Column widths are proportional to the column's symbol count (with a
      minimum width); group bands above follow those widths, stacked by depth.
- [x] Cells fill their column width; their height reflects their size; rows are layers.
- [x] Collapse/expand, hover, selection, impact and pan/zoom work as in the other modes.
- [x] Normal and dense modes are unchanged.
- [x] `layout.ts` / `mode.ts` tests cover the flame geometry; vitest, svelte-check and build pass.

## Out of scope
- Call-depth flame graph from entry points; hierarchical layers (rows).
