---
id: 18
title: Matrix view can be zoomed and panned
status: done
parent: null
branch: 18-matrix-zoom-pan
remote: 15
created: 2026-11-06
---
## Context

The matrix SVG in the centre of the app (`matrixView` in app/ui/src/main.ts)
has no navigation: normal mode stretches the whole viewBox to the pane, dense
mode keeps its natural pixel size and relies on the pane's scrollbars. On large
projects you can neither zoom into a region nor zoom out to see everything.
The user wants the basics of map-like navigation: zoom in, zoom out, move
sideways.

## Acceptance criteria
- [x] Mouse wheel / trackpad pinch (wheel with ctrlKey) zooms in and out
      around the cursor; the point under the cursor stays put.
- [x] Dragging with the primary button on the background or on a cell pans the
      view; trackpad two-finger scroll (wheel without ctrlKey) pans too.
- [x] A drag does not count as a click: clicking a cell (movement under a few
      pixels) still selects it and fills the side panel; hover colouring still works.
- [x] Toolbar buttons: zoom in (+), zoom out (−) around the centre, and
      "Fit" which shows the whole matrix. Zoom is clamped (e.g. 0.1×–10× of fit).
- [x] Works in both normal and dense modes, done by changing the SVG viewBox
      (the pane no longer needs scrollbars). Initial view: normal = fit;
      dense = 1:1 pixel scale anchored top-left (as today), Fit still available.
      Switching mode resets the view; impact/hover repaints keep it.
- [x] Cursor shows grab/grabbing while panning.
- [x] The viewBox maths (zoom around a point, pan by a screen delta, fit,
      clamp) is a pure module with vitest tests; `npm test` and
      `npm run build` pass in app/ui.

## Out of scope
- Keyboard navigation, minimap, persisting the view, zoom-dependent level of
  detail (e.g. hiding labels when zoomed out), touch gestures beyond what the
  wheel event gives, any Rust/API change.
