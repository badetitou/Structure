---
id: 38
title: Analysis screen, panels and history sidebar are Svelte components
status: done
parent: 36
branch: null
remote: null
created: 2026-11-06
---
## Context

After #37, port the analysis screen out of `main.ts`: progress view,
header/toolbar (dense mode, advanced options), the right details panel,
the history sidebar (commit graph via `lanes.ts`/`graphsvg.ts`) and the
resizable/collapsible panels (`panels.ts`). State shared between them
(project, current analysis, selection, impact) lives in Svelte stores /
runes modules, not module-level `let`s.

## Acceptance criteria
- [ ] Built from shadcn-svelte components (Button, Toggle, Resizable, ScrollArea, Tooltip, …) where one fits; each of: Progress, Toolbar, DetailsPanel, HistorySidebar, SidePanel
      (handle + collapse) is its own component using theme tokens.
- [ ] One app-state module holds project/analysis/selection; API calls go
      through `api.ts` only.
- [ ] Behaviour unchanged: panel widths/collapse persisted, history
      selection triggers impact, dense mode persisted.
- [ ] Existing vitest tests still pass; build and check pass.

## Out of scope
- The matrix SVG itself (#39).
