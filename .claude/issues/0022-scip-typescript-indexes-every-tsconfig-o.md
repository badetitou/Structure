---
id: 22
title: scip-typescript indexes every tsconfig of a large repository
status: done
parent: 20
branch: 22-scip-ts-projects
remote: 19
created: 2026-11-06
---
## Context

See #20. `pipeline.rs:425-431` runs scip-typescript with no project args → only the
root tsconfig. Timeout fixed at 600 s (`pipeline.rs:339`), then silent fallback to
tree-sitter only (`pipeline.rs:444`). No Node heap setting. Progress is time-based for
this indexer (`pipeline.rs:170`).

## Acceptance criteria
- [ ] Every tsconfig.json that is a project root (not under node_modules/ignored dirs)
  is passed to scip-typescript, or `--infer-tsconfig` when there is none.
- [ ] Timeout configurable (CLI flag / env), and a timeout is reported as an error or a
  visible warning, never a silent fallback.
- [ ] `NODE_OPTIONS=--max-old-space-size=<n>` set for the indexer (configurable).
- [ ] Unit tests on tsconfig discovery; angular-realworld golden unchanged.

## Out of scope
- Running the indexer on vscode in tests (the user does it).
