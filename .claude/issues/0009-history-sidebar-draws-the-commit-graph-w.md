---
id: 9
title: History sidebar draws the commit graph with branch lanes
status: done
parent: null
branch: 9-history-graph
remote: null
created: 2026-11-06
---
## Context

Follow-up of #4/#6. The history sidebar (`app/ui/src/main.ts` `renderHistory`,
`app/ui/src/history.ts`) is a flat list. Users expect a VS Code–style graph on
the left of the rows: one coloured lane per line of history, a dot per commit,
curves where branches fork and merge back.

## Acceptance criteria
- [ ] `history` lists commits in topological order (`git log --topo-order`), so
      lanes don't cross needlessly; documented in `docs/api.md`.
- [ ] Pure lane layout in TS (`app/ui/src/lanes.ts`): from `commits[].id/parents`
      → per row the commit's lane and colour, and the segments to draw between
      this row and the next (straight, fork, merge). Parents beyond the loaded
      commits run off the bottom. Vitest tests: linear, one branch + merge,
      two parallel branches, octopus/second-parent order, truncated parent.
- [ ] Each sidebar row shows the graph column (SVG) left of the text; the
      "Working tree" row shows a hollow dot above HEAD. Rows keep a fixed height
      so lines connect; click/shift-click selection still works.
- [ ] `docs/samples/history.json` has a branch and a merge so the browser preview
      shows the graph.
- [ ] tsc, vitest, cargo fmt/clippy/test green.

## Out of scope
- Showing branches not reachable from HEAD (`--all`).
- Collapsing/expanding merged branches, pagination.
