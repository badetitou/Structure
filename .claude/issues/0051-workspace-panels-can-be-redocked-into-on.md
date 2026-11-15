---
id: 51
title: Workspace panels can be redocked into one another reliably
status: done
parent: 46
branch: claude/dock-views-redocking-826f08
remote: 39
created: 2026-11-09
---
## Context

Part of #46, after #48. In the Tauri app, dragging a workspace tab did not dock it
anywhere: dockview defaulted to HTML5 drag-and-drop for the mouse, and the Tauri
webview (WKWebView) handles native drags itself, so the drags never reached dockview.

## Acceptance criteria
- [x] Dockview runs with `dndStrategy: "pointer"` (`workspace/dock.ts`, `dockDnd`),
      so docking uses pointer events and works in the Tauri webview.
- [x] A panel can be dropped into another group as a tab, on one of its sides
      (split), on a tab header, or on an edge of the whole workspace (checked in the browser).
- [x] `npm test`, `npm run build` (svelte-check + vite) pass.

## Out of scope
- The aim-at-a-cell DnD compass (dockview-enterprise only).
- Cross-window drags / popouts.
