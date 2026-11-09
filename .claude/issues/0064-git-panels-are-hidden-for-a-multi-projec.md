---
id: 64
title: Git panels are hidden for a multi-project analysis even when grouped with other panels
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

Seen in the browser check of #60 (`/?sample=matrix-multi`): the Diff tab stays visible next to
Details for a multi-project analysis ("Select a commit in the history to see its diff"), although
#59 makes it unavailable. `dock.ts syncVisibility` hides a group only when *all* its panels are
unavailable, so an unavailable panel sharing a group with an available one stays as a tab. The
same already happened for a single project outside git.

## Acceptance criteria
- [ ] Unavailable panels are not shown as tabs (hidden or removed and restored when available again),
  without losing the persisted layout.
- [ ] vitest for `syncVisibility` with a mixed group; checked in the browser with `?sample=matrix-multi`.
