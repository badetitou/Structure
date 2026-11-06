---
id: 43
title: Analysis has a benchmark and its hot spots are optimised
status: todo
parent: 36
branch: null
remote: null
created: 2026-11-06
---
## Context

Analysis speed on large projects (vscode fixture) is the target: 23-link-memory
already halved peak RSS. #21 already reports time and peak memory per step in the CLI; use it as
the benchmark (extend it only if a phase is still lumped together) and fix the top
hot spots it shows (e.g. allocations/cloning in link and classify, serial
loops that can use rayon, repeated string work).

## Acceptance criteria
- [ ] Per-phase numbers come from #21's CLI report (extended if needed).
- [ ] Before/after numbers on the Java fixture and vscode in the report;
      at least the slowest phase improved, no output change (golden + e2e).
- [ ] fmt, clippy, test pass.

## Out of scope
- Changing the indexers themselves; caching analysis results across runs.
