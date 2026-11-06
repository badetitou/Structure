---
id: 6
title: History sidebar selects the diff to analyse
status: done
parent: 4
branch: 6-history-sidebar
remote: null
created: 2026-11-06
---
## Context

Part of #4, after #5. `app/ui/src/main.ts` `matrixView` has a toolbar with
`base`/`head` inputs and a right `aside` panel.

## Acceptance criteria
- [ ] Left sidebar listing "Working tree" (if dirty) and commits from `api.history`.
- [ ] Click → `api.impact(path, commit.base, commit.id)`; "Working tree" → `impact(path, null, null)`.
      Selected entry highlighted; Clear resets it.
- [ ] Shift-click a second commit → range `older.base .. newer.id`.
- [ ] History load error → sidebar hidden, matrix unaffected.
- [ ] The free-text base/head inputs stay, folded under an "Advanced" toggle.
- [ ] Pure selection logic (range ordering, base choice) in its own module with vitest tests.

## Out of scope
- Branch lanes, textual diff, pagination.
