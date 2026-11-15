---
id: 79
title: Impact is computed from a unified diff text
status: done
parent: 76
branch: 79-impact-from-diff-text
remote: null
created: 2026-11-13
---
## Context

`structure-git::impact::changed_lines` shells out to `git diff -U0` and parses the
result. In the browser (#76) the diff comes from GitHub
(`Accept: application/vnd.github.diff` on the pull request), a unified diff with three
context lines, and there is no `git` process.

## Acceptance criteria
- [ ] `structure-git` exposes a pure function turning a unified diff text into
  `Vec<FileChange>` (new-side changed line ranges, same convention as `changed_lines`),
  correct with any number of context lines: context lines are not reported as changed.
- [ ] Handles added files, deleted files (no new-side lines), renames, binary files,
  pure deletions inside a hunk (same convention as today's `-U0` parsing) and
  `\ No newline at end of file`.
- [ ] Unit tests cover each case above, plus one test proving a `-U3` and a `-U0` diff
  of the same change give the same `FileChange`s.
- [ ] `changed_lines` behaviour unchanged; fmt, clippy `-D warnings`, tests green.

## Out of scope
- Fetching the diff; wasm build.
