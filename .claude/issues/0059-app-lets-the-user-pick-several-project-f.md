---
id: 59
title: App lets the user pick several project folders
status: done
parent: 54
branch: 59-app-multi-folders
remote: null
created: 2026-11-09
---
## Context

Tauri `analyze(path: String)` (app/src-tauri/src/commands.rs) and the UI open
one folder; the cache, history, impact, diff and cell_symbols are keyed by one
canonical path. `pipeline::analyze_many` (#55) analyses several roots.

## Acceptance criteria
- [x] `analyze` takes `paths: Vec<String>`; one path behaves exactly as today. Several paths
  run `analyze_many`, cached under a key made of the sorted canonical paths.
- [x] `cell_symbols` works on a multi-project analysis (same key scheme).
- [x] Home lets the user add several folders to one analysis (add / remove), and a recent
  entry remembers its set of folders; old single-path recents still load.
- [x] Git-based panels (history, impact, diff) are unavailable for a multi-project analysis
  (registry `available`), with no backend call made.
- [x] `ProjectHeader` shows the project names.
- [x] `docs/api.md` updated; vitest, svelte-check and `cargo test` in app/src-tauri green.

## Out of scope
- Drawing HTTP edges (#60). Git history/impact across several repos. Saved named workspaces.

## Brief (implementer)

Goal: the desktop app analyses several folders together (front + back), using `pipeline::analyze_many`.
Read first: app/README.md; app/src-tauri/src/commands.rs (`canonical`, `run_analyze`, `analyze`,
`cell_symbols`, cache `Entry`/key, fingerprint); crates/core/src/pipeline/multi.rs (`analyze_many`
signature); app/ui/src/api.ts (`analyze`, `pickFolder`, `WireMatrix`/`Matrix` — add `projects?`
and `http_edges?` types only); app/ui/src/recent.ts (+ test); app/ui/src/Home.svelte;
app/ui/src/state.svelte.ts (`app.project`); app/ui/src/workspace/registry.ts + builtin.ts
(`available`); app/ui/src/lib/components/ProjectHeader.svelte; docs/api.md.
Design: key = sorted canonical paths joined with "\n" (one path → unchanged key). Fingerprint:
combine per-root fingerprints. `app.project` holds `paths: string[]` (keep a `path` getter = first
for existing single-path code). Recent entries: `{paths, name, lastOpened}`; migrate `{path}` on load.
Multi-project = `paths.length > 1` → history/impact/diff panels `available: false`.
Verify: `cd app/ui && npm test && npm run check`; `cd app/src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`;
workspace `cargo test --workspace` still green. Do NOT run `cargo tauri dev` or npm install of new packages.
Rules: no new deps; no unwrap/expect outside tests; don't draw http_edges (#60).

## Known limits
- Not yet run in the live app (`cargo tauri dev`); unit-tested only.
- `matrix.project`/`language` come from the alphabetically first root.
