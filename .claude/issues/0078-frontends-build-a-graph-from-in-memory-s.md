---
id: 78
title: Frontends build a graph from in-memory sources
status: done
parent: 76
branch: 78-graph-from-memory-sources
remote: null
created: 2026-11-13
---
## Context

`lang-java::parse_project` and `lang-ts::parse_project` walk a directory and read files
themselves. In the browser (#76) there is no filesystem: sources arrive as
`(relative path, content)` pairs fetched from GitHub. The graph assembly (type index,
edge resolution, endpoints) must be reachable without a directory walk.

## Acceptance criteria
- [ ] `lang-java` and `lang-ts` each expose a function building a `Graph` from a list of
  `(relative path, source)` pairs, applying the same file filters as the directory walk
  (Java: `src/test/` excluded; TS: `.d.ts`, `.spec.ts`, `.test.ts(x)` and ignored
  directories excluded).
- [ ] `parse_project` (and `parse_project_with_warnings`) delegate to it: one code path,
  identical output — existing tests and goldens pass unchanged.
- [ ] A test per language builds a graph from in-memory sources and compares it to
  `parse_project` on the same files written to a temp dir.
- [ ] fmt, clippy `-D warnings`, tests green.

## Out of scope
- Java generated sources (`target/generated-sources`) in the in-memory path: none.
- wasm build, language detection.
