---
id: 44
title: History clicks show analysis progress and are blocked while a commit is analysed
status: in-review
parent: null
branch: 44-impact-busy-feedback
remote: 33
created: 2026-11-09
---
## Context

Clicking a commit in the history sidebar runs `impact`, which may check out and
analyse the commit's snapshot (can take minutes). The backend already emits
`analysis-progress` events during it, but the UI ignores them: the matrix stays
as is with no feedback, and the user can click another commit meanwhile
(the earlier answer is dropped, but the backend still runs both in turn).

## Acceptance criteria
- [x] While an impact runs, the matrix area shows a busy overlay with the
      step, percentage and detail line of the `analysis-progress` events.
- [x] History rows, "Show impact" and "Clear" are disabled while it runs;
      `pickHistory` / `runTypedImpact` ignore calls while busy.
- [x] The row being computed is marked as pending in the sidebar.
- [x] vitest covers: busy blocks a second pick, progress is reported and reset.
- [x] `npm run check`, vitest and the cargo checks pass.

## Out of scope
- Cancelling a running analysis in the backend.
