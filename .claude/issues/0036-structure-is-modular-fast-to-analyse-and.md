---
id: 36
title: Structure is modular, fast to analyse and its UI is themable and maintainable
status: done
parent: null
branch: null
remote: null
created: 2026-11-06
---
## Context

The user wants Structure more modular and efficient. Today `structure-core`
is one crate where `pipeline.rs` (~1300 lines) mixes indexer processes, git
history/snapshots and analysis, and each language is a `match Language::`
in ~15 places. The UI is vanilla TS: `main.ts` (~840 lines) builds the DOM
by hand with hardcoded colours. The pure UI modules (layout, lanes,
viewport, domaintree, panels, recent, mode, cellstate, matrixsel) are
already tested and framework-agnostic: they are kept.

Decisions: Svelte 5 + shadcn-svelte (Tailwind v4) for the UI; "efficient" = analysis speed, UI fluidity
and maintenance cost.

## Sub-issues (order)

- UI: #37 → #38 → #39
- Core: #40 → #41 → #42; #43 after #40 (independent of #41/#42)
- The two tracks are independent.

## Out of scope
- New features, TS/Java analysis changes, API contract changes (`docs/api.md`).
