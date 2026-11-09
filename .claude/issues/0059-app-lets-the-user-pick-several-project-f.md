---
id: 59
title: App lets the user pick several project folders
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

Tauri `analyze(path: String)` (app/src-tauri/src/commands.rs) and the UI open
one folder. History entries are per path.

## Acceptance criteria
- [ ] `analyze` takes a list of paths; the UI lets the user add several folders
  to one analysis; history keys an entry by its set of roots.
- [ ] Http edges can be shown/hidden in the matrix like other edge kinds.
- [ ] `docs/api.md` and `docs/samples` updated; vitest green.

## Out of scope
- Saving named multi-project workspaces.
