---
id: 80
title: Core analyses a pull request in WebAssembly
status: done
parent: 76
branch: 80-structure-wasm
remote: null
created: 2026-11-13
---
## Context

With #77, #78 and #79 the analysis no longer needs a filesystem or `git`. This issue
packages it for the browser (#76): one WebAssembly module that takes the sources of a
PR head and the PR diff and returns the impact with its matrix.

`structure-core` cannot be the entry point: it pulls `structure-scip` (protobuf, libc)
and process spawning. Default rules live in `rules/*.toml` and are wired per language in
`crates/core/src/lang/{java,typescript}.rs` (`default_rules`, `module_separator`).

## Acceptance criteria
- [ ] New crate `crates/wasm` (`structure-wasm`, `cdylib` + `rlib`) depending on `ir`,
  `lang-java`, `lang-ts`, `classify`, `git` only. New dependency: `wasm-bindgen`
  (JS glue) — say why in the report if more is needed.
- [ ] Exported `analyze_pr(files, diff, base, head)`: `files` = `[{path, content}]`,
  `diff` = unified diff text. Returns the `Impact` JSON with `matrix` set (same shape as
  `docs/samples/impact.json` + `matrix`). Language picked from the file extensions
  (Java or TypeScript; the majority language when both are present), default rules and
  module separator of that language.
- [ ] Exported `cell_symbols` equivalent, so the viewer can list the symbols of a cell.
- [ ] Native unit tests on the pure Rust entry point (no wasm needed to test the logic).
- [ ] `scripts/build-wasm.sh` builds the module for `wasm32-unknown-unknown` with the
  toolchain settings recorded in #76, and a Node smoke test loads it and analyses a
  two-file sample. `rayon` and `ignore` must not break the wasm build or panic at
  runtime (single-threaded fallback, or cfg-gated).
- [ ] `cargo test --workspace` on the host is unaffected; fmt, clippy green.

## Out of scope
- SCIP, multi-root, HTTP / RPC links.
- Fetching anything from GitHub.
