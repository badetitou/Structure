---
id: 56
title: Analysis screen has one compact top bar and a context status bar
status: done
parent: null
branch: claude/compact-menu-status-bar-bc3520
remote: null
created: 2026-11-09
---
## Context

The analysis screen stacks three bars above the dock (app header, project
header, toolbar), ~140px. The user wants the dock to take much more of the
window, and context shown in a status bar at the bottom.

## Acceptance criteria
- [x] One top bar (~36px) on the analysis screen: logo + project name, matrix
      controls (zoom, mode, swap axes, impact range, clear), Views, theme, close.
- [x] Typed impact range (base/head) moves into a popover.
- [x] Bottom status bar: path, language, files/symbols/edges/time, shown
      snapshot (working tree or commit), impact summary or progress, selected
      cell, impact legend.
- [x] Status bar labels come from a pure, unit-tested helper.
- [x] Home screen unchanged.

## Out of scope
- New panels, keyboard shortcuts, persisting status bar settings.
