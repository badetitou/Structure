---
id: 44
title: Analysis screen is a tiling workspace of plugin panels
status: in-progress
parent: null
branch: null
remote: null
created: 2026-11-09
---
## Context

The analysis screen has a fixed layout: history sidebar (left), matrix,
details panel (right), each side panel resizable/collapsible (#26,
`SidePanel.svelte`, `panels.ts`). The user wants a VS Code-like workspace:
free tiling (a recursive horizontal/vertical split tree, the matrix being one
tile among others) and panels contributed like plugins through an internal
TypeScript registry. The current right panel (details) and the history
sidebar become contributions.

## Acceptance criteria
- [ ] #45 registry + split-tree layout, replaces SidePanel.
- [ ] #46 split / tab / move / close / reopen panels.

## Out of scope
- Runtime-loaded external plugins (panels stay compiled into the UI).
- Any Rust/API change.
