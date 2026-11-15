---
id: 76
title: Browser extension shows a pull request's impact matrix on GitHub
status: in-progress
parent: null
branch: null
remote: null
created: 2026-11-13
---
## Context

Structure only runs as a desktop app or CLI on a local clone. Reviewers want the
`layer × domain` matrix of a pull request where they review it: on github.com, with
nothing to install but a browser extension. An extension cannot run the native binary,
so the analysis runs in the browser: the core is compiled to WebAssembly, the sources of
the PR head and the PR diff are read through the GitHub API.

Spike (2026-11-13, throwaway crate): tree-sitter 0.27 + tree-sitter-java 0.23.5 +
tree-sitter-typescript 0.23.2 build for `wasm32-unknown-unknown` and parse Java and TS
in Node (2 MB module, no imports). It needs, on macOS: Homebrew LLVM
(`CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`,
`AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar` — Apple `ar` archives
are silently not linked) and
`CFLAGS_wasm32_unknown_unknown=-I<tree-sitter-language crate>/wasm/include`.
tree-sitter 0.25 (current) does not build for that target.

## Acceptance criteria
- [ ] On `https://github.com/<owner>/<repo>/pull/<n>` of a public Java or TypeScript
  repository, the extension adds a "Structure" tab; opening it shows the matrix at the
  PR head with changed and impacted cells highlighted, without any local install
  besides the extension.
- [ ] Sub-issues #77–#82 are done.

## Out of scope
- Call edges (SCIP): tree-sitter only, impact follows `Uses` / supertype edges.
- Multi-root analysis, HTTP / RPC links, history and diff panels.
- Firefox / Safari packaging, store publication.
- Results computed in CI (GitHub Action) or by a local binary.
