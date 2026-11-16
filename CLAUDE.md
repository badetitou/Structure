# Structure — notes for Claude

Structure analyses a codebase (Java first, then TS) into a call graph,
quotients it into `layer × domain` cells and renders it as an interactive
matrix. POC plan and task split: `docs/plan.md`. Rust (cargo), local only — no remote yet.

## Commands

- `cargo check --workspace` — fast typecheck
- `cargo fmt --check` — formatting (run `cargo fmt` to fix)
- `cargo clippy --workspace --all-targets -- -D warnings` — lints, must be clean
- `cargo test --workspace` — all tests; `cargo test <name>` for one
- `cargo run -p structure-cli -- analyze <path>` / `cargo build --release`
- `UPDATE_TS_TYPES=1 cargo test -p structure-ir generated_ts_types` — regenerate
  `app/ui/src/generated/` (TS wire types) after changing a type in `crates/ir`;
  `cargo test` fails while they are stale.
- CI: `.github/workflows/ci.yml` (fmt, clippy, tests, vitest, svelte-check; no Tauri app, no wasm build)

## Map

- `Cargo.toml` — virtual workspace (edition 2024); `app/` (Tauri) is excluded.
- `crates/` — one-way deps `ir` <- `lang-java`/`lang-ts`/`lang-pharo`/`scip`/`classify`/`highlight` <- `git` <- `core` <- `cli`
  (`git` needs only `ir` and `highlight`; the others go straight to `core`):
  - `ir` (`structure-ir`): `ir` (Graph/Symbol/Edge), `model`
    (Layer/Matrix/Impact/Progress), `highlight` (Anchor/Seed/HighlightRequest/
    Highlight) — contracts, serde only; feature `ts`
    derives the TS declarations (ts-rs; module `ts`, test `generated_ts_types`).
  - `lang-pharo` (`structure-lang-pharo`): hand-written Tonel reader (classes,
    methods, superclass and `Uses` edges; no `Call` edges yet).
  - `lang-java`, `lang-ts` (`structure-lang-*`): tree-sitter frontends (symbols,
    ranges, annotations, supertype and `Uses` edges; `ext:<Name>` = external type).
  - `scip` (`structure-scip`): `scip` reads `index.scip`; `link` joins SCIP
    occurrences to tree-sitter symbols → `Call`/`Uses`/`Implements` edges;
    `metrics` (run stats).
  - `classify` (`structure-classify`): `classify` + `rules/*.toml` (embedded
    layer/domain rules, per-language default rules and module separator);
    `matrix` — quotient graph.
  - `highlight` (`structure-highlight`): pure (wasm-safe) highlight model over a
    graph: `HighlightIndex`, `resolve` (anchors → top-level symbols),
    `propagate` (callers/callees BFS), `aggregate` (→ cells), `highlight`.
  - `git` (`structure-git`): `impact` (git diff → `diff_request`, a `git-diff`
    highlight → `Impact::from_highlight`; `ImpactIndex` = `HighlightIndex`) and
    ref/fingerprint helpers (`resolve_base`, `resolve_commit`, `worktree_fingerprint`).
  - `core` (`structure-core`): facade, re-exports the crates above under their old
    module names (`ir`, `model`, `java`, `ts`, `scip`, `link`, `metrics`, `classify`,
    `matrix`, `impact`, `highlight`). Own code: `lang/` (`LanguageFrontend` trait, registry
    `all()`/`frontend()`, Java/TS indexer + detection) and `pipeline/` (`mod`
    analyze/detect, `indexer`, `process`, `progress`, `snapshot`, `history`) and
    `http_link` (matches TS `http_calls` to Java `http` endpoints → `Http` edges,
    multi-root only) and `rpc_link` (matches GWT `rpc_calls` to the classes
    implementing the sync service → `Rpc` edges, one or several roots).
- `crates/cli` — bin `structure` (`analyze`, `impact`). Keep `main` thin.
- `app/` — Tauri 2 app: `src-tauri` (commands over `pipeline`), `ui` (Vite + Svelte
  5 + TS, vitest): `src/api.ts` backend wrappers over the generated wire types
  in `src/generated/` (never hand-edited), `src/*.ts` state/geometry helpers, `src/workspace/` panel registry +
  dockview glue, `src/lib/components` Svelte components (`matrix/`, `workspace/`, `ui/`). See `app/README.md`.
- `docs/` — `plan.md`, `api.md` (Tauri API contract), `samples/*.json`.
- `scripts/` — fetch and index fixtures; `tests/golden/` — expected classifications.
- `fixtures/` (gitignored) — sample projects. Tests needing them read env
  `STRUCTURE_FIXTURES` (default `<workspace>/fixtures`) and skip when absent.
  The user clones new fixtures and runs their builds/indexers: auto mode refuses
  agents that clone or build outside code, so briefs never ask for it.
- JDKs: default 21. JDK 17 is the Homebrew keg
  `/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home`;
  `/usr/libexec/java_home -v 17` wrongly returns JDK 27 here.

## Rules

1. Before calling a change done: `cargo fmt --check`, clippy with
   `-D warnings`, and `cargo test` all pass.
2. No `unwrap()`/`expect()` outside tests and `main` — propagate errors with `?`.
3. No `unsafe` without a `// SAFETY:` comment and the user's agreement.
4. New dependencies only when asked or clearly justified; say why in the report.
5. One change = one concern. Refactors go in their own change.
6. A result with a target metric (link mapping ratio, golden accuracy) is
   reported only once it meets the target; below it, keep improving or say
   exactly what blocks it — no "still low, I'll keep you posted".

## Workflow

- Team and retro history: `.claude/team.md`.
- Start a task with the `task-prep` skill. Issues live on GitHub
  (`badetitou/Structure`, `github-flow` skill): English issues, branch
  `<issue>-<slug>`, PR with `Closes #N`. `.claude/issues/*.md` is the archived
  local tracker (its `remote:` field = GitHub issue number); don't add to it.
- Commit only when asked. No Claude attribution in commits.
