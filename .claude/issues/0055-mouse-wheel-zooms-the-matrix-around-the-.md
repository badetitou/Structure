---
id: 55
title: Mouse wheel zooms the matrix around the cursor
status: done
parent: null
branch: claude/scroll-zoom-matrice-120db4
remote: 43
created: 2026-11-09
---
## Context

In the matrix view a plain wheel / two-finger scroll pans and only a pinch
(ctrl+wheel) zooms. The user wants the scroll wheel to zoom in and out.

## Acceptance criteria
- [x] Plain wheel zooms around the cursor (wheel up = zoom in), with the same
      clamps as the toolbar buttons (`zoomAtClamped`).
- [x] Pinch (ctrl+wheel) still zooms around the cursor.
- [x] Shift+wheel pans; drag-to-pan unchanged.
- [x] One mouse notch zooms by a moderate step (~1.2×), not ×2.7.
- [x] Wheel → zoom factor is a pure helper in `viewport.ts`, unit-tested.

## Out of scope
- Zoom animation / smoothing, a setting to switch back to scroll-pans.

Note: first committed as #54 (id clash with the transpose issue); commit 0af69c9 says "Refs #54".
