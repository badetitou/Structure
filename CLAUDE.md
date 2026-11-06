# Structure — notes for Claude

Structure analyses a codebase (Java first, then TS) into a call graph,
quotients it into `layer × domain` cells and renders it as an interactive
matrix. POC plan and task split: `docs/plan.md`. Rust (cargo), local only — no remote yet.

## Commands

- `cargo check` — fast typecheck
- `cargo fmt --check` — formatting (run `cargo fmt` to fix)
- `cargo clippy --all-targets -- -D warnings` — lints, must be clean
- `cargo test` — all tests; `cargo test <name>` for one
- `cargo run` / `cargo build --release` — run / build

## Map

- `Cargo.toml` — binary crate `structure`, edition 2024, no dependencies yet.
- `src/main.rs` — entry point. Keep `main` thin; put logic in modules
  (`src/<module>.rs`) so it can be unit-tested.
- `tests/` — integration tests (create when needed).
<Update this list as modules appear.>

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
