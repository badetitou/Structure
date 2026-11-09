---
id: 67
title: Git panels are hidden for a multi-project analysis even when grouped with other panels
status: done
parent: 57
branch: 64-hide-unavailable-panels
remote: null
created: 2026-11-09
---
## Context

Seen in the browser check of #63 (`/?sample=matrix-multi`): the Diff tab stays visible next to
Details for a multi-project analysis ("Select a commit in the history to see its diff"), although
#62 makes it unavailable. `dock.ts syncVisibility` hides a group only when *all* its panels are
unavailable, so an unavailable panel sharing a group with an available one stays as a tab. The
same already happened for a single project outside git.

## Acceptance criteria
- [x] Unavailable panels are not shown as tabs (hidden or removed and restored when available again),
  without losing the persisted layout.
- [x] vitest for `syncVisibility` with a mixed group; checked in the browser with `?sample=matrix-multi`.

## Brief (implementer)

Goal: a panel whose `available()` is false never shows as a tab, even when it shares a dockview group with available panels,
and it comes back when it becomes available. The persisted layout (`structure.dock`) must not lose it.
Read first: app/README.md (workspace section), app/ui/src/workspace/dock.ts (`syncVisibility`, layout persistence),
workspace/registry.ts, workspace/builtin.ts (`available` for history/diff: `!app.multi && app.history !== null`),
lib/components/workspace/Workspace.svelte, MissingPanel.svelte, ViewsMenu.svelte, and the existing dock tests.
Check dockview's API in node_modules/dockview-core for per-panel hiding (e.g. removing the panel and re-adding it at its saved position,
or a group/panel `setVisible`). Pick the simplest approach that restores the panel in the same group and position, and explain it.
The Views menu must not offer an unavailable panel, or must show it disabled.
Verify: `cd app/ui && npm test && npm run check && npm run build` (`npm ci` first if node_modules is missing). Unit test syncVisibility
(or its replacement) with a mixed group [details (available), diff (unavailable)] → diff hidden, details shown; then diff available → restored
in the same group; layout saved without dropping diff. Single-project behaviour with git (all panels available) must be unchanged.
Rules: no new deps.

## Result
- Browser (`?sample=matrix-multi`): History and Diff tabs hidden with 2 projects; back with a single project (jpetstore sample); `structure.dock` still holds `diff`.
- Known limit: hiding is an inline style on the dockview tab (`data-tab-panel-id`), re-applied on layout changes.
