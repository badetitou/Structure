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

## Map

- `Cargo.toml` — virtual workspace (edition 2024); `app/` (Tauri) is excluded.
- `crates/core` — lib `structure-core`:
  - `ir` (Graph/Symbol/Edge), `model` (Layer/Matrix/Impact/Progress) — contracts.
  - `java`, `ts` — tree-sitter frontends (symbols, ranges, annotations, supertype
    and `Uses` edges; `ext:<Name>` = external type).
  - `scip` — reads `index.scip`; `link` — joins SCIP occurrences to tree-sitter
    symbols → `Call`/`Uses`/`Implements` edges.
  - `classify` + `rules/*.toml` — declarative layer/domain rules.
  - `matrix` — quotient graph; `impact` — git diff → changed/impacted cells.
  - `pipeline` — detect language, run/reuse indexer, analyze, impact.
- `crates/cli` — bin `structure` (`analyze`, `impact`). Keep `main` thin.
- `app/` — Tauri 2 app: `src-tauri` (commands over `pipeline`), `ui` (Vite + TS,
  vitest). See `app/README.md`.
- `docs/` — `plan.md`, `api.md` (Tauri API contract), `samples/*.json`.
- `scripts/` — fetch and index fixtures; `tests/golden/` — expected classifications.
- `fixtures/` (gitignored) — sample projects. Tests needing them read env
  `STRUCTURE_FIXTURES` (default `<workspace>/fixtures`) and skip when absent.

## Rules

1. Before calling a change done: `cargo fmt --check`, clippy with
   `-D warnings`, and `cargo test` all pass.
2. No `unwrap()`/`expect()` outside tests and `main` — propagate errors with `?`.
3. No `unsafe` without a `// SAFETY:` comment and the user's agreement.
4. New dependencies only when asked or clearly justified; say why in the report.
5. One change = one concern. Refactors go in their own change.

## Workflow

- Team and retro history: `.claude/team.md`.
- Start a task with the `task-prep` skill. No remote yet: skip GitHub issues
  and the project board; one task → one local branch `<slug>`.
- Commit only when asked. No Claude attribution in commits.
