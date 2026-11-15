---
id: 40
title: pipeline.rs is split into indexer, git and analysis modules
status: done
parent: 36
branch: 40-pipeline-split
remote: 29
created: 2026-11-06
---
## Context

`crates/core/src/pipeline.rs` (~1300 lines) mixes: indexer discovery and
process running (find_cs, run_indexer, progress parsing, line readers,
tsconfig discovery, node heap), git history and snapshots (history,
snapshot, prune_snapshots, impact_at), and orchestration (detect_language,
ensure_index, analyze, impact). Pure refactor: split into
`pipeline/{mod,indexer,progress,snapshot,history}.rs` or similar.

## Acceptance criteria
- [x] No behaviour change; public API of `structure_core::pipeline` unchanged
      (re-exports), CLI and Tauri compile untouched.
- [x] No file in the new module over ~400 lines; tests moved next to their code.
- [x] fmt, clippy -D warnings, cargo test pass.

## Out of scope
- Language trait (#41), crate split (#42), any optimisation.
