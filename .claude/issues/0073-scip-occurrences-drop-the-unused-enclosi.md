---
id: 73
title: SCIP occurrences drop the unused enclosing range
status: done
parent: null
branch: 73-drop-enclosing-range
remote: null
created: 2026-11-09
---
## Context

After #72 a `ScipOccurrence` is 44 bytes, 20 of them an `enclosing_range:
Option<Range>` that only tests read (145k of vscode's 8.8M occurrences have one).
~168 MiB of the vscode index for nothing.

## Acceptance criteria
- [x] `ScipOccurrence::enclosing_range` removed; decoding still accepts occurrences
  that carry one (roundtrip test keeps it in its input).
- [x] Linked graph, `LinkStats` and matrix fingerprints identical to #72 on
  jpetstore-6, angular-realworld and vscode.
- [x] vscode: occurrence 44 → 24 bytes; live index 896 → 727 MiB; link allocation
  peak 1470 → 1302 MiB; max RSS of `structure impact` ~2.57 → ~2.36 GB; time unchanged.
- [x] fmt, clippy `-D warnings`, `cargo test --workspace` (also with fixtures)
  green; Tauri app compiles.

## Out of scope
- Parsing each distinct symbol name once in `link`.
