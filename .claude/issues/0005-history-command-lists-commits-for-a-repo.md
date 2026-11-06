---
id: 5
title: History command lists commits for a repository
status: done
parent: 4
branch: 5-history-command
remote: null
created: 2026-11-06
---
## Context

Part of #4. The UI needs the commit list of the opened project. Git is already
invoked through `git()` in `crates/core/src/impact.rs`.

## Acceptance criteria
- [ ] `pipeline::history(path, limit) -> History` (or in a new `history` module):
      `{ dirty: bool, commits: Commit[] }`, `Commit = { id, short, parents: string[],
      base: string, subject, author, date (ISO 8601), refs: string[] }`.
- [ ] `base` is the diff base to pass to `impact`: first parent, or the empty tree
      (`git hash-object -t tree /dev/null`) for a root commit; `impact(base, id)` works for both.
- [ ] Not a git repo → a clear error (UI uses it to hide the sidebar).
- [ ] Tauri command `history(path, limit)` registered; `docs/api.md` documents it;
      `docs/samples/history.json` added for the browser fallback.
- [ ] Unit tests on a temp git repo (root commit, merge, dirty tree, subject with
      tabs/newline-safe separator); fmt, clippy -D warnings, tests green.

## Out of scope
- UI (#6). Branch lanes.
