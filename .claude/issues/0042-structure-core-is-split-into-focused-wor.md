---
id: 42
title: structure-core is split into focused workspace crates
status: done
parent: 36
branch: 42-crate-split
remote: 31
created: 2026-11-06
---
## Context

After #40 and #41, split `structure-core` into crates with one-way deps:
`structure-ir` (ir, model) ← `structure-lang-java`, `structure-lang-ts`,
`structure-scip` (scip + link), `structure-classify` (+ rules),
`structure-git` (impact, history, snapshots) ← `structure-core` (pipeline
facade re-exporting what CLI/Tauri use). Heavy deps (tree-sitter grammars,
protobuf) only in the crates that need them, so rebuilds stay local.

## Acceptance criteria
- [ ] Workspace members updated; CLI and `app/src-tauri` build with no code
      change beyond imports.
- [ ] `cargo tree` shows no cycle and no grammar dep in ir/classify/git.
- [ ] CLAUDE.md "Map" updated.
- [ ] fmt, clippy, test pass.

## Out of scope
- Behaviour changes, publishing crates.
