---
id: 20
title: Structure analyses the VS Code repository on a laptop
status: in-progress
parent: null
branch: null
remote: null
created: 2026-11-06
---
## Context

We want to study large codebases such as microsoft/vscode (~6k TS files, ~1.5M LOC,
many tsconfigs under `src/` and `extensions/`). A read-only review of the code
(2026-11-06, estimates, not measurements) found: scip-typescript indexes only the root
tsconfig and is killed after 600 s with a silent tree-sitter fallback; SCIP load keeps
~3× the index in memory; linking allocates two owned Strings per reference edge, sorts
them all, and expands interface calls by an unmemoized DFS; the matrix payload lists
every top-level symbol per cell; "Show impact" re-runs the whole analysis; the UI
redraws every cell on hover and lists symbols without virtualization.

## Acceptance criteria
- [ ] #21: baseline on vscode — time and peak RSS per phase, recorded in this issue.
- [ ] #22: the index covers vscode's TS projects; no silent timeout.
- [ ] #23: analysis with a reused index meets the target set from the baseline
  (initial target: ≤ 2 min and ≤ 4 GB peak RSS on vscode).
- [ ] #24: matrix payload ≤ 2 MB on vscode; impact does not re-analyse.
- [ ] #25: first matrix render and hover stay under ~100 ms per frame on vscode.
- [ ] Results on jpetstore / realworld fixtures and goldens unchanged.

## Out of scope
- Domain classification for vscode (`src/vs/<area>`): handled by the fragmented
  name-root fallback (#16/#17); revisit only if the baseline shows it fails.
- Incremental re-indexing.

## Baseline (2026-11-06, vscode c0a88d240be, release build, M-series 16 cores / 64 GB)

Tree-sitter only (`--no-scip`; no index yet):
- 10 583 files, 212 457 symbols, 59 873 edges (2 684 to `ext:`).
- parse 1 434 ms, classify 828 ms, matrix 76 ms — total 2.3 s; peak RSS 553 MiB.
- Matrix JSON 18.9 MB: 3 layers (`model`, `util`, `unknown`), 1 974 domains,
  2 684 cells, 13 478 cell edges; `domain_tree` present.
- Takeaways: Rust parse/classify is not the bottleneck; the payload (#24) is, and
  vscode's own layering (`base/platform/editor/workbench`, `common/browser/node/electron-*`)
  is not recognised — classification follow-up to open.

Indexer (run by the user, `src/tsconfig.json` only, no node_modules):
141 s, peak RSS 12.6 GB (Node), index.scip 1.01 GB.

With SCIP (index reused):
- 11 472 documents, 8.84 M occurrences, 2.66 M references → 1.77 M edges
  (call 960 k, uses 612 k, implements 30 k, extends 9 k; 158 k to `ext:`).
- Link: 163 352 mapped, 98 772 lifted, 15 174 unmapped definitions.
- indexer 0.4 s, parse 1.5 s, scip load 2.2 s, link 6.3 s, classify 1.1 s,
  matrix 1.2 s — **total 12.9 s** (time target met); **peak RSS 6.9 GiB** (target 4 GB: not met).
- Matrix JSON 33 MB: 3 layers, 1 974 domains, 2 501 cells, 133 514 cell edges.

Next: #23 (memory: SCIP load ~1 GB index → GBs of owned Strings, 1.77 M edges with
String ids) and #24 (payload: 33 MB, 133 k cell edges).

## After #22–#24 (main 3e35ae0, vscode, index reused)
- total 11.8 s, peak RSS 3.5 GiB (≤ 4 GB target met; was 6.9 GiB) — #23 streams SCIP documents.
- Matrix payload 1.70 MB compact (≤ 2 MB target met; was 24.7 MB) — #24 cell summaries + grouped edges.
- Remaining: #25 (UI responsiveness, after #17/#18), #35 (vscode layers/domains).
- Follow-ups from #24 review, not done: dedupe `impact_in_snapshot` with `impact`;
  move the Tauri cache to `cache.rs`; `cell_symbols` scans all symbols per page.
