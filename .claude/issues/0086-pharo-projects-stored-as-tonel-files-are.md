---
id: 86
title: Pharo projects stored as Tonel files are analysed into the matrix
status: in-progress
parent: null
branch: null
remote: null
created: 2026-11-16
---
## Context

Structure analyses Java and TypeScript. Pharo (Smalltalk) projects kept in git as
Tonel files (`src/<Package>/<Class>.class.st`, `*.extension.st`, `package.st`) should
get the same `layer × domain` matrix. Pharo has no SCIP indexer and is dynamically
typed, so the frontend builds the whole graph itself (tree-sitter-only path:
`ensure_index` returns `None` when `indexer_command` is `None`).

Decisions (2026-11-16): Tonel only (v1 and v3); call edges by a precision-first
heuristic (#88), ambiguous sends give no edge.

## Acceptance criteria
- [x] #87 — parsing, symbols, inheritance, class references, frontend registered.
- [ ] #88 — message sends become `Call` edges.
- [ ] `structure analyze <pharo repo>` renders a usable matrix in the app.

## Out of scope
- FileTree / Metacello `.filetree` layouts, live image via MCP.
- Baseline (`BaselineOf…`) dependency parsing, git impact specifics.
