---
id: 49
title: Diff panel shows the changes of the selected commit
status: in-review
parent: null
branch: 49-commit-diff-panel
remote: null
created: 2026-11-09
---
## Context

Picking a commit (or a range, or the working tree) in the history sidebar
shows its impact on the matrix and the changed symbols, but not the change
itself. A "Diff" panel in the dockview workspace shows the textual diff of
the current history selection.

## Acceptance criteria
- [x] Tauri command `diff(path, base, head)` (same `base`/`head` semantics as
      `impact`) returns the changed files with status (added / deleted /
      modified / renamed), +/- counts, binary flag and hunks of lines with old /
      new line numbers. Untracked files appear as added for the working tree.
      Large diffs are truncated (per file and in total) and flagged. Documented in `docs/api.md`.
- [x] The diff parser lives in `structure-git` and is unit-tested (modify,
      add, delete, rename, binary, header look-alikes in hunks, truncation).
- [x] A "Diff" panel is registered in the workspace (available with history),
      shown as a tab next to Details in the default layout; it lists files
      (collapsible, +/- counts) with coloured added / removed lines and line numbers.
- [x] Selecting a commit, a range or the working tree in the history loads
      its diff; clearing the selection empties the panel; late answers are ignored.
- [x] Browser dev mode serves `docs/samples/diff.json`.
- [x] `cargo fmt --check`, clippy `-D warnings`, `cargo test`, `npm test`,
      `npm run check`, `npm run build` pass.

## Out of scope
- Syntax highlighting, side-by-side view, word-level diff.
- Linking diff lines to matrix cells / symbols.
