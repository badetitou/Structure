---
id: 77
title: Workspace builds on tree-sitter 0.27
status: done
parent: 76
branch: 77-tree-sitter-0-27
remote: null
created: 2026-11-13
---
## Context

The browser extension (#76) needs the frontends compiled to `wasm32-unknown-unknown`.
tree-sitter 0.25 has no C headers for that target (`'stdio.h' file not found`);
0.27 ships them. The upgrade is its own change so a parsing regression is easy to spot.

## Acceptance criteria
- [ ] Workspace `tree-sitter` is `0.27`; `lang-java` and `lang-ts` compile against it
  (grammar crates stay at their current versions unless they must move).
- [ ] No behaviour change: existing tests and goldens pass unchanged.
- [ ] `cargo fmt --check`, clippy `-D warnings`, `cargo test --workspace` green.

## Out of scope
- Any wasm build, any new API.
