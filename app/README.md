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
`npm test` runs vitest; `npm run check` runs svelte-check (`.ts` and `.svelte`; `npm run build` runs it too). Hovering a cell temporarily overrides impact colouring. The "HTTP links" toolbar toggle (kept in localStorage as `structure.httpEdges`, shown only when the matrix has front-to-back http links) shows or hides the dashed link edges; in a plain browser `/?sample=matrix-multi` loads `docs/samples/matrix-multi.json`. The "Dense" toolbar toggle (kept in localStorage) packs the matrix into uniform tiles without labels or edges (`layout.ts`).

## Layout

- `src-tauri/` crate `structure-app` (standalone workspace, own Cargo.lock/target).
- `ui/` Vite + Svelte 5 + Tailwind v4 + shadcn-svelte. `App.svelte`/`Home.svelte`/`ThemeSwitch.svelte` are the shell; the analysis screen is `lib/components/AnalysisScreen.svelte` (Progress, ProjectHeader, Toolbar, then the `workspace/Workspace.svelte` docking workspace) over one runes state module, `src/state.svelte.ts` (`app`: project, analysis, selection, impact, history, mode; backend calls via `api.ts`). The workspace is [dockview](https://dockview.dev) (tabs, drag-and-drop docking, resizable groups): `workspace/Workspace.svelte` creates it, `src/workspace/dock.ts` mounts registered Svelte panels (`SvelteRenderer`), hides groups whose panels are unavailable, and persists the layout under `structure.dock`; its colours are mapped to the theme tokens in `app.css` (`.dockview-theme-structure`). Panels are contributions registered in `src/workspace/registry.ts` (`panels.register({ id, title, component, available? })`). The built-ins (`matrix` → `MatrixPanel`, `details` → `DetailsPanel`, `history` → `HistorySidebar`) and the default layout are in `src/workspace/builtin.ts`; a new panel is one more `register` call, reads what it needs from `app`, and shows up in the toolbar's Views menu. The matrix is a component tree: `MatrixView.svelte` (derives columns/layout/bands from the matrix, mode and expanded groups; owns zoom/pan as one `transform`) over `matrix/ColumnHeaders`, `matrix/MatrixCell`, `matrix/MatrixEdge`. Hover/impact/selection are per-cell/edge derivations (`cellstate.ts`, adjacency built once in `graph.ts`); the containers carry `hovering`/`impacting` classes that dim the unaffected cells and edges, so a hover only writes to the ones that change. Long symbol lists in the details panel show 200 at a time ("Show more"). `src/app.css` is the single theme file (shadcn variables for light and `.dark`, plus matrix/impact/lane tokens); no colour literals elsewhere, SVG uses `var(--…)` via `svgattr.ts`. Light/dark/system is handled by `mode-watcher`. Add a component with `cd ui && npx shadcn-svelte@latest add <name>` (lands in `src/lib/components/ui`, alias `$lib`). `typescript` is pinned to ^6 because `svelte-check` does not support TS 7 yet. `src/api.ts` typed wrappers for the API in `docs/api.md`.
