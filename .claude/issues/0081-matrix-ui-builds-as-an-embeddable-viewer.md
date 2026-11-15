---
id: 81
title: Matrix UI builds as an embeddable viewer fed with a matrix and an impact
status: done
parent: 76
branch: 81-embeddable-viewer
remote: null
created: 2026-11-13
---
## Context

The matrix UI lives in `app/ui` and gets its data through `app/ui/src/api.ts`, the only
module importing `@tauri-apps/*` (it already falls back to `/samples/*.json` outside
Tauri). The extension (#76) needs the same matrix, fed with an `Impact` + `Matrix`
computed by the wasm module, without Tauri, history, diff or project picking.

## Acceptance criteria
- [ ] `api.ts` goes through a backend interface (analyze, impact, cell symbols…); the
  Tauri implementation is today's behaviour, unchanged for the desktop app.
- [ ] A second Vite entry builds a standalone viewer page that renders the matrix from a
  `Matrix` + `Impact` handed to it (injected backend / `postMessage`), with changed and
  impacted cells highlighted and the cell symbol list working through the backend.
- [ ] Panels needing git (history, diff) and the home screen are absent from the viewer.
- [ ] vitest, svelte-check and both builds green; viewer checked in a browser with
  `docs/samples` data.

## Out of scope
- The extension itself, GitHub calls, wasm.
