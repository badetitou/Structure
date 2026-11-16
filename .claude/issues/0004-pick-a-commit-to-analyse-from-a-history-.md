---
id: 4
title: Pick a commit to analyse from a history sidebar
status: done
parent: null
branch: null
remote: 4
created: 2026-11-06
---
## Context

Today the matrix view picks a diff through two free-text inputs (`base`, `head`)
in `app/ui/src/main.ts`. The user has to know or copy commit ids. When a git
project is opened, a side panel should show the recent history so a diff is
picked with one click.

## Acceptance criteria
- [ ] Opening a git project shows a history sidebar: a "Working tree" entry
      (when there are uncommitted changes) then recent commits of `HEAD`
      (short id, subject, author, relative date, branch/tag labels).
- [ ] Clicking a commit shows the impact of that commit (parent → commit);
      clicking "Working tree" shows HEAD → working tree.
- [ ] Shift-click on a second commit selects a range (older → newer).
- [ ] Root commits and merge commits (first parent) work.
- [ ] A non-git folder hides the sidebar; the matrix still works.

## Slices
1. #5 — `history` command in core + Tauri + `docs/api.md`.
2. #6 — UI sidebar using it (depends on #5).

## Out of scope
- Drawing branch lanes (graph view of all branches); can follow as its own issue.
- Per-file diff tree / showing the textual diff.
- Pagination beyond a fixed limit.
