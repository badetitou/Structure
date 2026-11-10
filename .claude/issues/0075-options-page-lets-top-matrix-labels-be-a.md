---
id: 75
title: Options page lets top matrix labels be angled at 45 degrees
status: done
parent: null
branch: 75-options-angled-labels
remote: null
created: 2026-11-09
---
## Context

Dense matrix headers on top are rotated -90°: they are hard to read (head tilted) and
take as much height as the longest label. The app has no place for display preferences
beyond the top-bar toggles.

## Acceptance criteria
- [x] An Options page (gear button in the home bar and the analysis top bar, Escape / close
  button to leave) groups app preferences; the theme choice is there too.
- [x] Option "Top labels: Vertical / 45°" (persisted, default vertical). At 45°, labels
  that are rotated along the top (dense: domains, or layers when transposed) are drawn
  at -45°, anchored on their column; the header room and the matrix width grow so
  nothing is cut.
- [x] Vertical = today's matrix exactly (existing layout tests unchanged).
- [x] vitest, svelte-check, build green; checked in the browser.

## Out of scope
- Other options; angled labels in normal / flame mode (labels there are horizontal).
