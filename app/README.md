# Structure desktop app (Tauri 2)

Skeleton with MOCKED commands (`analyze`, `impact` return `docs/samples/*.json`).
To go real, edit only `src-tauri/src/commands.rs`: replace the `mock::*` calls.

Prerequisites: Rust toolchain, `cargo install tauri-cli --version "^2"`, Node 20+,
platform webview deps (macOS: none extra).

## Run

```
cd app/ui && npm install
cd ../src-tauri && cargo tauri dev        # full desktop app
cd app/ui && npm run dev                  # browser only, uses /samples/*.json
cd app/src-tauri && cargo tauri build --debug --no-bundle   # binary in target/debug/structure-app
```

`app/ui/public/samples/` is a copy of `docs/samples/` for the browser fallback
(used when `__TAURI_INTERNALS__` is absent); refresh it if the samples change.

## Layout

- `src-tauri/` crate `structure-app` (standalone workspace, own Cargo.lock/target).
- `ui/` Vite + TypeScript. `src/api.ts` typed wrappers for the API in `docs/api.md`.
