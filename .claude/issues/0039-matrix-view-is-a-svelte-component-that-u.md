---
id: 39
title: Matrix view is a Svelte component that updates incrementally
status: todo
parent: 36
branch: null
remote: null
created: 2026-11-06
---
## Context

Implements #25 (matrix UI stays responsive with thousands of cells) as part of the Svelte port: close #25 with this issue. Read #25 for the hot spots (per-cell SVG rebuild, paint() on hover, `graph.ts` reach(), unvirtualised symbol lists).


`matrixView` in `main.ts` rebuilds the whole SVG on every collapse/expand,
dense toggle or selection. Port it to a Svelte component tree (Matrix,
header bands, cell) driven by `layout.ts`, `viewport.ts`, `domaintree.ts`,
`cellstate.ts`, `matrixsel.ts`, so that selection/hover/zoom only update
the affected nodes, and pan/zoom touch one `transform`.

## Acceptance criteria
- [ ] `main.ts` is reduced to mounting `App.svelte` (no hand-built DOM left).
- [ ] Hover/selection does not re-render the matrix (verified with a counter
      or the Svelte devtools note in the report); pan/zoom updates a single
      transform.
- [ ] On the largest sample (`docs/samples`), collapse/expand and zoom stay
      fluid; report before/after timings in the branch report.
- [ ] Colours come from theme tokens; works in light and dark.
- [ ] Tests, build and check pass.

## Out of scope
- Canvas/WebGL rendering; new matrix features.
