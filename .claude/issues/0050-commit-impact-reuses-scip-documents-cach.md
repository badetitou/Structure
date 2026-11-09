---
id: 50
title: Commit impact reuses SCIP documents cached per file blob instead of re-running the indexer
status: done
parent: null
branch: 50-scip-blob-cache
remote: null
created: 2026-11-09
---
## Context

The impact of a commit is computed in a snapshot worktree, where the SCIP
indexer runs from scratch: 2.4 s (angular-realworld), 38 s (jpetstore, Maven),
minutes on vscode, and it can fail (build, missing `node_modules`). Tree-sitter
parsing and linking take milliseconds to ~3 s on vscode; the indexer is >99 %
of a cold snapshot analysis.

A SCIP document only depends on its file's content (plus, rarely, how its
dependencies resolve), and SCIP symbols are global names, not positions. So a
document produced for blob `b` at path `p` is valid in any commit where `p`
has blob `b`. Consecutive commits share almost all blobs.

## Acceptance criteria
- [x] Every analysis with a fresh `index.scip` in a git repository stores its
      documents in a content-addressed cache under
      `<git-common-dir>/structure/scip-docs/<indexer-version>/`, keyed by (blob sha, path) (only files
      not modified after the index was written). Done once per index (stamp).
- [x] A snapshot analysis without its own `index.scip` builds the SCIP index
      from cached documents for the commit's `(path, blob)` pairs and does not
      run the indexer, when at least one document is cached.
- [x] Files without a cached document fall back to tree-sitter only; a warning
      gives the coverage (`n of m source files`) and some uncovered paths.
- [x] `Options::exact` (CLI `impact --exact`) forces today's behaviour
      (indexer in the snapshot). CLI `impact --head` analyses the snapshot
      like the app does.
- [x] Tests: cache populated then reused for a commit; coverage warning;
      same blob at another path not reused; no cache outside git; files
      touched after the index skipped and a newer index re-stored; damaged
      entries; eol-converted (CRLF) checkouts. fmt,
      clippy `-D warnings`, tests green. (The exact path has no new test: it
      would run a real indexer; it is the unchanged `ensure_index` path.)

## Measurements (angular-realworld, release CLI)
- Cached commit impact: 160–290 ms vs 1.8–2.5 s with `--exact` (jpetstore:
  ~0.2 s vs ~38 s for the indexer).
- HEAD~15 with only the working tree's documents: 45/69 files covered,
  5 impacted symbols missed, all in files the warning listed.
- After one exact analysis of HEAD~15, HEAD~14 and HEAD~16 are covered
  69/69 and their impact equals `--exact`.

## Review (review-team)
- Fixed: unique temp names for concurrent writers; a compose error falls back
  to the indexer with a warning (damaged entries count as missing); blob ids
  hashed with attribute filters (CRLF repos hit); key includes the path and
  the indexer version; hashing moved to `structure-git`; `--exact` requires
  `--head`; one read per cached entry.
- Accepted: a file edited while the indexer runs may be stored with the older
  document (same as a stale index.scip).
- After the fixes, HEAD~13 and HEAD~17 (never analysed): 69/69 covered,
  impact identical to `--exact`, ~0.27 s vs ~2 s.

## Follow-ups
- Coverage counts every source file in the tree, including files the indexer
  never indexes (e2e outside tsconfig, `.mvn/wrapper`): count only paths some
  index has seen.
- Cache bound / eviction (vscode's index is ~1 GB).
- UI: show coverage, offer the exact analysis.

## Out of scope
- Cache size bound / eviction (follow-up).
- UI toggle between cached and exact analysis, coverage display (follow-up).
- Parsing a commit from blobs without a worktree.
