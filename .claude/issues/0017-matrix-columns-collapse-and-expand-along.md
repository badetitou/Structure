---
id: 17
title: Matrix columns collapse and expand along the domain tree
status: in-review
parent: 15
branch: 17-collapsible-domains
remote: null
created: 2026-11-06
---
## Context

See #15, #16. With `domain_tree`, the matrix can show groups of domains as one
aggregated column.

## Acceptance criteria
- [ ] When `domain_tree` is present, the first view expands the tree breadth-first
  while the column count stays ≤ 12; each column is a tree node.
- [ ] A group column aggregates its leaves: cell size = sum, symbols = union, edges
  re-mapped and summed (no self-loops), impact = min distance / changed if any leaf.
- [ ] Clicking a group header expands it; an expanded group can be collapsed again
  (header band above its columns). Pure logic in a tested module (vitest).
- [ ] `DomainNode` and `Matrix.domain_tree?` added to `app/ui/src/api.ts` per `docs/api.md`.
- [ ] Without `domain_tree`, the view is unchanged.

## Out of scope
- Changing the Rust contract (done in #16).
