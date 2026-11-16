---
id: 72
title: SCIP occurrences reference interned symbols
status: done
parent: null
branch: 72-scip-symbol-interning
remote: 60
created: 2026-11-09
---
## Context

The decoded SCIP index is the memory peak of an analysis. On vscode (965 MB
`index.scip`) it held ~1.9 GB: 8.8M `ScipOccurrence`s of 64 bytes each (539 MiB),
each owning its symbol `String` (639 MiB), for only 1.01M distinct symbols (112 MiB
of names). Symbol infos stored their name twice (map key and `symbol` field).

## Acceptance criteria
- [x] `ScipIndex` interns every symbol string once (`SymbolId`, `name`, `id`,
  `intern`, `symbol`); occurrences, symbol infos (keyed by id) and relationships
  hold ids. Documents are still decoded in parallel (symbols deduplicated per
  document, then interned in document order).
- [x] `link` output unchanged: linked graph, `LinkStats` and matrix fingerprints
  identical before/after on jpetstore-6, angular-realworld and vscode.
- [x] vscode: live index 1.9 GB → 0.75 GB; allocation peak during link
  2.63 GB → 1.47 GB; max RSS of `structure impact` 4.51 GB → ~2.57 GB; total
  5.4 s → ~4.8 s (load ~+0.1 s, link ~−0.15 s, index drop 0.28 → 0.06 s).
- [x] fmt, clippy `-D warnings`, `cargo test --workspace` (also with fixtures)
  green; Tauri app compiles.

## Out of scope
- `ScipOccurrence::enclosing_range` is read by nothing but tests (20 of the 44
  bytes of an occurrence, ~168 MiB on vscode): own change.
- Parsing each distinct symbol name once in `link` (top CPU hotspot): own change.
