---
id: 45
title: Panels are registered contributions laid out in a resizable split tree
status: in-review
parent: 44
branch: 45-tiling-panels
remote: null
created: 2026-11-09
---
## Context

Part of #44. Today `AnalysisScreen.svelte` hard-codes
`SidePanel(History) | MatrixView | SidePanel(Details)`.

## Acceptance criteria
- [x] A panel registry (`app/ui/src/workspace/registry.ts`): a contribution
      is `{ id, title, icon?, component, available?() }`; built-ins `matrix`,
      `details`, `history` register through it, nothing in the layout code
      names them.
- [x] A pure layout module (`app/ui/src/workspace/tiling.ts`): split tree
      (`split` nodes with direction + sizes, `leaf` nodes holding panel ids),
      resize, normalisation, parse/serialise with safe fallback; vitest tests.
- [x] The default tree reproduces today's layout: history | matrix | details.
- [x] Splitters are draggable (min size per tile), sizes persist across reloads
      (localStorage), invalid/unknown stored state falls back to the default.
- [x] A tile whose panel is unavailable (no history) takes no room.
- [x] Toolbar zoom/fit still drive the matrix; matrix re-fits on resize.
- [x] `SidePanel.svelte` and `panels.ts` are removed; `npm test`, `npm run build` pass.

## Out of scope
- Changing the tree from the UI (split, move, close): #46.
