# Structure desktop app (Tauri 2)

Commands `analyze` / `impact` call `structure_core::pipeline` (see
`src-tauri/src/commands.rs`); analyses are cached per canonical path in managed
state. Pipeline warnings are emitted as `analysis-progress` steps prefixed
`warning: `. In a plain browser the UI falls back to `docs/samples/*.json`.

Prerequisites: Rust toolchain, `cargo install tauri-cli --version "^2"`, Node 20+,
platform webview deps (macOS: none extra).

## Run

```
cd app/ui && npm install
cd ../src-tauri && cargo tauri dev        # full desktop app
cd app/ui && npm run dev                  # browser only, uses /samples/*.json
cd app/src-tauri && cargo tauri build --debug --no-bundle   # binary in target/debug/structure-app
```

`app/ui/public/samples/` is a git-ignored copy of `docs/samples/`, refreshed by
`predev`/`prebuild`; it is the browser fallback (no `__TAURI_INTERNALS__`).
`npm test` runs vitest; `npm run check` runs svelte-check (`.ts` and `.svelte`; `npm run build` runs it too). Hovering a cell temporarily overrides impact colouring. The "Dense" toolbar toggle (kept in localStorage) packs the matrix into uniform tiles without labels or edges (`layout.ts`).

## Layout

- `src-tauri/` crate `structure-app` (standalone workspace, own Cargo.lock/target).
- `ui/` Vite + Svelte 5 + Tailwind v4 + shadcn-svelte. `App.svelte`/`Home.svelte`/`ThemeSwitch.svelte` are the shell; the analysis screen is still vanilla DOM in `src/main.ts` (`runAnalysis(path, host, onExit)`), to be ported. `src/app.css` is the single theme file (shadcn variables for light and `.dark`, plus matrix/impact/lane tokens); no colour literals elsewhere, SVG uses `var(--…)` via `svgattr.ts`. Light/dark/system is handled by `mode-watcher`. Add a component with `cd ui && npx shadcn-svelte@latest add <name>` (lands in `src/lib/components/ui`, alias `$lib`). `typescript` is pinned to ^6 because `svelte-check` does not support TS 7 yet. `src/api.ts` typed wrappers for the API in `docs/api.md`.
