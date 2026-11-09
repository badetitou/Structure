---
id: 70
title: Working-tree impact no longer reads untracked files whole
status: done
parent: null
branch: 70-untracked-line-count
remote: null
created: 2026-11-09
---
## Context

`impact::changed_lines(project, None, None)` counts the lines of every untracked
file with `std::fs::read`, i.e. the whole file in memory. The SCIP index we write
into the project (`index.scip`) is untracked: on the vscode fixture (965 MB index)
every working-tree impact spends ~0.49 s and ~1 GB of transient memory reading it,
and the pipeline then drops it (`changes.retain(|c| c.file != "index.scip")`).
Measured with a counting allocator during the 2026-11-09 perf diagnosis.

## Acceptance criteria
- [x] `changed_lines` never opens `index.scip` (at the root or in a sub-directory,
  same rule as `history`'s dirty check) and does not report it.
- [x] Other untracked files are line-counted with a fixed-size buffer, same result
  as today (empty file = `(0, 1)`, no trailing newline counts as a line); only
  regular files are read, as in `diff::add_untracked`.
- [x] Regression test in `crates/git/tests/impact_git_modes.rs`.
- [x] fmt, clippy `-D warnings`, `cargo test --workspace` green; vscode
  `git changed_lines` re-measured.

## Out of scope
- The `retain(index.scip)` calls in `pipeline::history`/`snapshot` (still needed for a
  tracked index in a diff).
- `impact::compute` cost (~0.7 s, reverse-dependency map rebuilt per call): own issue.
