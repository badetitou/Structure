---
id: 24
title: Matrix payload sends cell summaries and impact reuses the cached graph
status: done
parent: 20
branch: 24-cell-summaries
remote: 21
created: 2026-11-06
---
## Context

See #20. `model.rs:67-88` / `matrix.rs:53-91` put every symbol (id, name, file) in each
Cell, sent in one Tauri invoke. `commands.rs:79` re-runs the full analysis on every
"Show impact"; `impact.rs:205-212` rebuilds callers over all edges each call.

## Acceptance criteria
- [ ] `docs/api.md` updated first: cells carry counts; a new command returns one cell's
  symbols (paged or capped).
- [ ] Impact reuses the cached Graph for the same project/commit.
- [ ] UI fetches a cell's symbols on click; existing views unchanged on fixtures.

## Out of scope
- UI rendering performance (#25).
