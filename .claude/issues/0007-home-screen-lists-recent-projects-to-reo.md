---
id: 7
title: Home screen lists recent projects to reopen an analysis quickly
status: in-review
parent: null
branch: 7-recent-projects
remote: null
created: 2026-11-06
---
## Context

The home screen (`home()` in `app/ui/src/main.ts`) only offers "Open project…"
then "Analyze". Every launch means browsing to the folder again. Nothing is
persisted today: no localStorage, no tauri-plugin-store, the analysis cache in
`commands.rs` is in memory. Re-analysing is cheap when the project's
`index.scip` is fresh, so "reopen" = run `api.analyze(path)` again.

## Acceptance criteria
- [x] A "Recent projects" list on the home screen, most recent first, max 8 entries,
      each showing the folder name and full path, plus last-analysed date.
- [x] Clicking an entry sets the project path and starts the analysis directly.
- [x] A path is recorded (deduplicated, moved to top) after a **successful** analysis only.
- [x] Each entry can be removed from the list; a failed reopen shows the error on
      home and keeps the entry (the user may remove it).
- [x] Storage in the UI via `localStorage` behind a small module
      (`app/ui/src/recent.ts`), read/write wrapped in try/catch; corrupt data → empty list.
- [x] Vitest unit tests for the module's pure logic (add, dedupe, cap, remove, parse
      of corrupt JSON) without a DOM dependency.
- [x] `npm test` and `npm run build` in `app/ui` green; Rust checks untouched.

## Out of scope
- Persisting analysis results on disk / instant reopen without re-analysing.
- Remembering the diff base or selected commit per project (#4, #6).
- Backend (Rust) storage or new Tauri plugins.
