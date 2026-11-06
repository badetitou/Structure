---
id: 37
title: UI runs on Svelte 5 and shadcn-svelte with light and dark themes
status: done
parent: 36
branch: 37-svelte-shadcn
remote: null
created: 2026-11-06
---
## Context

`app/ui` is Vite + vanilla TS. Switch it to Svelte 5 with **shadcn-svelte**
(Bits UI primitives, components copied into `src/lib/components/ui`,
Tailwind v4) so we use a maintained, accessible component kit instead of
hand-built widgets. Theming uses shadcn's CSS variables (`--background`,
`--foreground`, `--primary`, `--muted`, `--border`, …) in light and `.dark`
sets, extended with app tokens (matrix forward/backward colours, impact
ramp, history lane colours). Theme switch via `mode-watcher` (shadcn's
recommended light/dark/system helper).

A first attempt (branch `28-svelte-theme`, vanilla tokens, no kit) is
superseded; reuse its findings: the theme must be applied before mount (no
flash), SVG `<text>` needs `fill` from a token, the impact ramp in
`cellstate.ts` must come from tokens, `var()` in SVG attributes needs one
shared helper, the selected project path must survive Home remounts, and a
missing analysis host must fall back home with an error.

## Acceptance criteria
- [x] Svelte 5, Tailwind v4, shadcn-svelte (`components.json`) set up;
      `npm run build`, `npm test`, `npm run check` pass.
- [x] Every colour is a CSS variable in one theme file (shadcn base + app
      tokens), light and dark; no colour literal elsewhere (incl. `cellstate.ts`).
- [x] Light / dark / system switch in the header (mode-watcher), persisted,
      no flash of the wrong theme at startup.
- [x] App shell and Home (recent projects, open folder, analyze) are Svelte
      components built from shadcn components (Button, Card, …); the analysis
      screen still mounts via `main.ts` until #38.
- [x] Browser fallback (`npm run dev`) renders home and the sample analysis in
      both themes; Tauri dev starts; `app/README.md` updated.

## Out of scope
- Porting the analysis screen, panels, history, matrix (#38, #39).
