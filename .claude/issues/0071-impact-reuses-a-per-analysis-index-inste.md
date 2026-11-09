---
id: 71
title: Impact reuses a per-analysis index instead of rebuilding it
status: done
parent: null
branch: 71-impact-index
remote: null
created: 2026-11-09
---
## Context

`impact::compute` rebuilt, on every call, an id → symbol map, lifted every edge
endpoint to its top-level class as a fresh `String`, and built a
`HashMap<String, BTreeSet<String>>` of callers. On the vscode fixture (212k
symbols, 1.77M edges) that is ~0.65–0.70 s and 3.6M allocations per impact,
whatever the diff — paid again on every commit clicked in the app's history,
although the cached analysis does not change.

## Acceptance criteria
- [x] `ImpactIndex` (top-level symbol per symbol, symbols per file, deduplicated
  class callers as indices) built once per `Analysis`, on first use
  (`Analysis::impact_index`), reused by `pipeline::impact` and
  `impact_in_snapshot`.
- [x] `compute` keeps its signature (builds a one-off index); `compute_with` takes
  the index. Same results, including duplicate ids (last wins) and container cycles
  (never lifted); unit test for both.
- [x] vscode: impact output identical to before on 4 change sets (none, hub file
  whole, hub file lines, 4 mixed files); index build 240 ms once, then 0–11 ms per
  impact (was ~0.65–0.70 s each).
- [x] fmt, clippy `-D warnings`, `cargo test --workspace` (also with fixtures) green;
  Tauri app compiles.

## Out of scope
- Interned ids in `Graph` (would make the index build itself cheaper).
- `git ls-files --others` (~0.27 s on vscode) in `changed_lines`.
