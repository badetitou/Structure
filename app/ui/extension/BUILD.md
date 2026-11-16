# Building the Structure extension from source

These steps rebuild the submitted extension from the source archive
(`structure-extension-<version>-source.zip`). The release build runs them on
GitHub Actions (`.github/workflows/extension-release.yml`).

The extension bundles two kinds of generated code:

- `assets/*.js`, `content.js`, `*.html`: Svelte/TypeScript under `app/ui/src`
  bundled and minified by Vite.
- `assets/structure_wasm_bg-*.wasm` and its JS glue: the Rust crates under
  `crates/` compiled to WebAssembly by `wasm-pack` (crate `crates/wasm`).

## Environment

Ubuntu 24.04 (or macOS 14+) with:

- Rust stable 1.93 or later (`rustup`), target `wasm32-unknown-unknown`
- `wasm-pack` 0.15 or later
- clang and llvm-ar (LLVM 18 or later), for the tree-sitter C code
- Node.js 24 and npm 11
- Python 3 (used by `scripts/build-wasm.sh`)

On Ubuntu:

```sh
sudo apt-get install -y clang llvm python3 zip
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
export CC_wasm32_unknown_unknown="$(command -v clang)"
export AR_wasm32_unknown_unknown="$(command -v llvm-ar)"
```

On macOS: `brew install llvm` instead of the apt line; `scripts/build-wasm.sh`
finds Homebrew's LLVM by itself.

## Build

From the root of the unpacked source archive:

```sh
scripts/build-wasm.sh                 # → crates/wasm/pkg/
cd app/ui
npm ci
npm run build:extension               # → app/ui/dist-extension/
```

`app/ui/dist-extension/` is the extension, identical to the submitted package
(`cd dist-extension && zip -r -X ../structure.zip .`).
