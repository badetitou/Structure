---
id: 46
title: Panels can be split, tabbed, moved and reopened in the tiling workspace
status: todo
parent: 44
branch: null
remote: null
created: 2026-11-09
---
## Context

Part of #44, after #45. The split tree exists but its shape is fixed.

## Acceptance criteria
- [ ] A leaf can hold several panels shown as tabs; clicking a tab activates it.
- [ ] Dragging a tab onto a tile's edge splits that tile; onto its centre or tab
      bar moves it into that tile; emptied tiles disappear and the tree is normalised.
- [ ] A tab can be closed; a "Views" menu lists registered panels and reopens a
      closed one; "Reset layout" restores the default.
- [ ] Tree operations (insert at edge, move, close, normalise) are pure and tested.
- [ ] The layout persists as in #45; `npm test`, `npm run build` pass.

## Out of scope
- Floating windows, keyboard layout commands, runtime-loaded plugins.
