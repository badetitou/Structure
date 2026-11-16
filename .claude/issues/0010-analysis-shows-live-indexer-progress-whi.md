---
id: 10
title: Analysis shows live indexer progress while Maven or Gradle runs
status: done
parent: null
branch: 10-indexer-progress
remote: 10
created: 2026-11-06
---
## Context

When analysing a Java project, the UI stays on "indexing (5%)" for the whole
Maven/Gradle build run by scip-java, with no feedback (can take minutes).
`pipeline::run_indexer` (crates/core/src/pipeline.rs) discards stdout, only
buffers stderr for the error tail, and polls `try_wait` silently; `analyze`
emits a single `step("indexing", 5)` before it and `step("parsing", 35)` after.

## Acceptance criteria
- [x] While the indexer runs, `analyze` keeps emitting `Progress` events: the
      latest meaningful output line of the build (stdout and stderr, trimmed,
      length-capped) and the elapsed time, at least every ~1 s.
- [x] The percentage moves inside the indexing band (5..35) and never goes
      backwards: from Maven reactor counters (`Building X [3/12]`) when present,
      otherwise a slow time-based creep capped below 35.
- [x] `Progress` gains an optional `detail` field (omitted when absent);
      `docs/api.md`, `app/ui/src/api.ts` updated (no progress sample exists in `docs/samples`); the UI shows
      the detail line under the bar.
- [x] The error tail on failure and the timeout still work as today.
- [x] Unit tests for the line parser / pct logic (Maven reactor line, Gradle
      `> Task :x` line, noise lines, monotonic pct).

## Out of scope
- Progress for TypeScript indexing beyond what the generic line streaming gives.
- Cancelling an analysis from the UI.
- Progress bars for the parse/link steps.
