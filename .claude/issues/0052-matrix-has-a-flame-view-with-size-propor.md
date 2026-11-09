---
id: 52
title: Matrix has a flame view with size-proportional, deep domain columns
status: in-progress
parent: null
branch: claude/matrix-view-depth-horizontal-79fbf8
remote: null
created: 2026-11-09
---
## Context

The matrix caps the default domain expansion at 12 columns and gives every
column the same pitch, so the domain hierarchy (#16) stays shallow and the
diagram stays square. The user wants depth that lets it grow horizontally,
like a flame graph / icicle: the domain tree stacked over several levels,
each block as wide as its symbol count.

## Acceptance criteria
- [ ] A third matrix mode `flame` next to normal and dense (toolbar, persisted).
- [ ] In flame mode the domain tree starts expanded much deeper (no 12-column cap).
- [ ] Column widths are proportional to the column's symbol count (with a
      minimum width); group bands above follow those widths, stacked by depth.
- [ ] Cells fill their column width; their height reflects their size; rows are layers.
- [ ] Collapse/expand, hover, selection, impact and pan/zoom work as in the other modes.
- [ ] Normal and dense modes are unchanged.
- [ ] `layout.ts` / `mode.ts` tests cover the flame geometry; vitest, svelte-check and build pass.

## Out of scope
- Call-depth flame graph from entry points; hierarchical layers (rows).
