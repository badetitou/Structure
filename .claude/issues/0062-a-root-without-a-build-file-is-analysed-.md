---
id: 62
title: A root without a build file is analysed as its sub-projects
status: in-progress
parent: 54
branch: 62-split-undetectable-root
remote: null
created: 2026-11-09
---
## Context

`structure analyze /…/WeMagnus/wegf` fails: "cannot detect language" — the root holds
`backend/` (Maven), `frontend/` (Nx, package.json) and `we-elements/` (TS) but no build
file itself. The user had to pass `wegf/backend wegf/frontend` by hand.

## Acceptance criteria
- [ ] A root with no build file of its own whose direct children (depth 1, then 2 if none)
  are detectable projects is analysed as those sub-projects, exactly as if they were passed
  as several roots (#55), names = child dir names.
- [ ] A root that is itself detectable keeps today's behaviour (no splitting), so Maven/Gradle
  multi-module and Nx roots are unchanged.
- [ ] CLI and app both benefit (pipeline level); the CLI prints the detected projects.
- [ ] Test on a temp dir layout; wegf root gives backend + frontend + we-elements and the same
  http stats as passing backend + frontend (+ we-elements).

## Out of scope
- Mixed roots that are themselves a project and also contain another language's project
  (e.g. a Maven root with a `frontend/` package.json inside) — note as follow-up.

## Brief (implementer)

Goal: `structure analyze /…/WeMagnus/wegf` (and the app's analyze of that folder) works by analysing its
detectable sub-projects together, exactly like passing them as several roots.
Read first: crates/core/src/lang/mod.rs (`detect_language`, `frontend`, the per-language `detect`), crates/core/src/lang/{java,typescript}.rs
(detect rules), crates/core/src/pipeline/mod.rs (`analyze`, `analyze_in`, the "cannot detect language" error) and
pipeline/multi.rs (`analyze_many`: one root delegates to `analyze`; names from dir names; HTTP/RPC linking on the merged graph),
crates/cli/src/main.rs, app/src-tauri/src/commands.rs (`canonical_many`, `run_analyze`).
Real layout, read-only: /Users/benoit.verhaeghe/Documents/WeMagnus/wegf has `backend/` (pom.xml), `frontend/` (package.json, Nx),
`we-elements/` (package.json + tsconfig.json), `database/` (no build file), no build file at the root.
Design:
- New `pipeline::expand_roots(&[PathBuf]) -> Result<Vec<PathBuf>>`: a root that is itself detectable is kept as is.
  An undetectable root is replaced by its detectable direct children, sorted by name. If there are none, it uses its
  detectable grandchildren. Skip hidden dirs, `node_modules`, `target`, `build`, `dist`. No detectable child at either
  depth → today's "cannot detect language" error, with a hint listing what was looked at.
- `analyze_many` calls it first (so CLI, app, and single-path `analyze` of an undetectable root all benefit). Guard
  against recursion (one root after expansion → plain `analyze`).
- Project names are the child dir names (`backend`, `frontend`, `we-elements`), with the existing de-duplication.
- The CLI prints the detected projects (already does per project). The app needs no change if it goes through
  `analyze_many`; check that `canonical_many` and the cache key still use the user's paths.
Measure (read-only; never build or write there): `cargo build --release -p structure-cli`;
`./target/release/structure analyze --no-scip /Users/benoit.verhaeghe/Documents/WeMagnus/wegf -o <scratch>`. It must list
the 3 projects. Its `http:` line must equal that of `analyze --no-scip …/wegf/backend …/wegf/frontend …/wegf/we-elements`,
which you run too. Compare with the 2-root run (1783/2043).
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace with
STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures; app/src-tauri cargo test + clippy.
Unit tests with tempdirs: undetectable root → children, depth-2 fallback, detectable root kept (a Maven multi-module or Nx root
is NOT split), skipped dirs, nothing found → error, mixed list of a detectable root plus an undetectable one.
Rules: no unwrap/expect outside tests; no new deps.
