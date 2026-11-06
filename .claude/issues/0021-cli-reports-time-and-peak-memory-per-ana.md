---
id: 21
title: CLI reports time and peak memory per analysis phase
status: done
parent: 20
branch: 21-phase-metrics
remote: null
created: 2026-11-06
---
## Context

See #20. We need numbers before optimizing. `crates/cli/src/main.rs:58-67` already
prints ms per step, but the "linking" step (`pipeline.rs:503-519`) lumps `scip::load`
and `link` together and nothing reports memory.

## Acceptance criteria
- [ ] `structure analyze` reports separate timings for: indexer (or reuse), parse,
  scip load, link, classify, matrix, and the counts that drive cost (files, symbols,
  SCIP documents/occurrences, edges by kind incl. `ext:`).
- [ ] Peak RSS printed at the end (getrusage, no new dependency if possible).
- [ ] The user runs it on vscode (index provided by the user) and the numbers go in #20.

## Out of scope
- Any optimization.
