---
id: 58
title: Several project roots are analysed into one matrix
status: done
parent: 57
branch: 55-multi-root-analysis
remote: null
created: 2026-11-09
---
## Context

`analyze` takes one path and one language (crates/core/src/pipeline/mod.rs,
crates/core/src/lang/mod.rs `detect_language`). Symbol ids have no project
prefix, so two projects could collide. `Graph.language` / `Matrix.language`
and `Matrix.project` are singular (crates/ir/src/model.rs).

## Acceptance criteria
- [x] `structure analyze <path>...` accepts one or more roots; one root behaves
  exactly as today (same JSON, goldens unchanged).
- [x] Each root is detected, indexed (SCIP cache per root) and classified with
  its own frontend and rules, then merged into one graph.
- [x] With several roots, symbol ids are prefixed by a project name (root dir
  name, de-duplicated) so they cannot collide, and each cell's domain is
  qualified by its project (e.g. `front/article`), so front and back stay apart.
- [x] `Matrix` lists its projects (name, root, language); single-language
  fields stay for compatibility or are documented as changed in `docs/api.md`.
- [x] Test: realworld pair analysed together gives the union of both cell sets.

## Out of scope
- HTTP extraction and links (#59–#61); app UI (#62); progress per project
  beyond prefixing messages.

## Brief (implementer)

Goal: `structure analyze <path>...` analyses several roots into one merged
graph and matrix; one root = today's output, byte for byte.
Read first: crates/core/src/pipeline/mod.rs (`analyze`, `analyze_in`, detect,
classify step), crates/core/src/lang/mod.rs (`detect_language`),
crates/ir/src/ir.rs (Graph, ids), crates/ir/src/model.rs (Matrix, Cell id
`layer:domain`), crates/classify/src/matrix.rs (`build`), crates/cli/src/main.rs.
Design: run the existing per-root pipeline up to classification for each root
(own language, SCIP cache, rules), then merge: prefix ids `<project>::` and
domains `<project>/` only when there are ≥ 2 roots; project name = root dir
name, suffixed `-2`… on clash. Add `projects: Vec<Project{name, root, language}>`
to Matrix (serde default, skipped when empty) — keep `language`/`project` as
the first root's for compatibility. Leave the Tauri command single-path (#62).
Verify: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings;
cargo test --workspace (with STRUCTURE_FIXTURES=/Users/benoit.verhaeghe/Documents/Perso/Structure/fixtures
so goldens run); then `cargo run -p structure-cli -- analyze $F/spring-boot-realworld-example-app $F/angular-realworld`.
Done: criteria of #58 ticked, goldens unchanged, new test for the two-root merge.
Rules: no unwrap/expect outside tests; no new deps; don't touch HTTP extraction.
Fixtures are already cloned and indexed by the user — never clone or build them.
