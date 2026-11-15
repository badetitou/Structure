---
id: 26
title: Side panels can be collapsed and resized
status: done
parent: null
branch: 26-side-panels
remote: 23
created: 2026-11-06
---
## Context

The analysis screen (`app/ui/src/main.ts`, around `const panel = h("aside")`)
has two fixed-width side panels around the matrix: the history sidebar on the
left (`nav.history`, 260px, hidden until history loads) and the details panel on
the right (`aside`, 340px). The matrix pane cannot get more room, and long
symbol names or commit subjects cannot get more width. The user wants both
panels collapsible and resizable.

## Acceptance criteria
- [x] Each side panel has a drag handle on its inner edge (left panel: right
      edge, right panel: left edge); dragging it resizes the panel. Width is
      clamped (e.g. 160px min, ~60% of the window max).
- [x] Each panel has a toggle (chevron button on the handle or in the toolbar)
      that collapses it to a thin strip and expands it back to its last width.
      Double-clicking the handle also toggles.
- [x] Collapsing/resizing does not select text or start a matrix pan; the
      matrix pane re-fits through the existing ResizeObserver (no distortion).
- [x] The history panel stays hidden while there is no history, as today;
      once shown, it is collapsible/resizable like the right panel.
- [x] Widths and collapsed state are remembered across reloads (localStorage,
      same pattern as the dense-mode setting), with a safe fallback when
      storage is unavailable or values are invalid.
- [x] Width clamping and stored-state parsing live in a small pure module with
      vitest tests; `npm test` and `npm run build` pass in app/ui.

## Out of scope
- Moving panels to other sides, more panels, keyboard resizing, any Rust/API
  change, restyling panel contents.
