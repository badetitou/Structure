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
`npm test` runs vitest. Hovering a cell temporarily overrides impact colouring.

## Layout

- `src-tauri/` crate `structure-app` (standalone workspace, own Cargo.lock/target).
- `ui/` Vite + TypeScript. `src/api.ts` typed wrappers for the API in `docs/api.md`.
