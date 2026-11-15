---
id: 54
title: Matrix can be transposed (domains as rows, layers as columns)
status: done
parent: null
branch: claude/matrix-inversion-d4b021
remote: 42
created: 2026-11-09
---
## Context

The matrix always shows layers as rows and domains as columns. With many
domains (flame view, deep trees) the matrix grows very wide; the user wants to
flip it and read domains top to bottom.

## Acceptance criteria
- [x] A toolbar toggle swaps the axes: layers become columns, domains rows.
- [x] Works in normal, dense and flame views; flame bars grow from the left.
- [x] Domain group bands move to the left of the domain labels and still
  collapse / expand on click; labels stay horizontal and readable.
- [x] Edges, hover, impact and selection behave as before.
- [x] The choice is persisted like the view mode; switching resets the viewport.
- [x] Unit tests on the transposed layout.

## Out of scope
- Reordering layers or domains.
