---
id: 48
title: Workspace runs on dockview with tabs, docking and a Views menu
status: done
parent: 46
branch: 46-tile-tabs
remote: 36
created: 2026-11-09
---
## Context

Merged in 1ed0a97; commit 7b30a90 refers to it as #46 (renumbered: the id was taken on main).

Part of #46, after #47. #47 laid panels out in an in-house split tree. Rather
than also writing tabs and drag-and-drop docking ourselves, the workspace moves
to dockview (popular, zero-dependency, vanilla TS docking engine: tabs, groups,
HTML5 drag-and-drop to split / merge, floating groups, toJSON/fromJSON). The
panel registry from #47 stays the plugin API; dockview replaces `tiling.ts`.

## Acceptance criteria
- [x] `dockview` (pinned 8.7.1, `dockview-core` overridden to the same) renders
      the workspace; registered Svelte panels are mounted through a small adapter
      (`workspace/dock.ts`, `SvelteRenderer`). The in-house tiling code is removed.
- [x] Panels can be grouped as tabs, dragged to split / merge groups, resized and
      closed (dockview); a panel whose plugin is gone is dropped from a restored layout.
- [x] Default layout: history | matrix | details (260 / rest / 340 px).
- [x] A group holding only unavailable panels (history without git) is hidden and
      comes back when the panel becomes available.
- [x] A "Views" menu in the toolbar lists registered panels (checked = open),
      toggles them, and has "Reset layout".
- [x] Layout persisted in localStorage (`structure.dock`), invalid data → default.
- [x] Dockview colours come from the app theme tokens (light / dark).
- [x] Pure helpers tested; `npm test`, `npm run check`, `npm run build` pass.

## Out of scope
- Popout windows (need a popout URL / Tauri windows), keyboard layout commands,
  runtime-loaded plugins.
