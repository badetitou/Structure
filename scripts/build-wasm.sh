#!/usr/bin/env bash
# Build crates/wasm for wasm32-unknown-unknown and generate the JS glue in
# crates/wasm/pkg (wasm-pack, --target web). Respects already-set CC/AR/CFLAGS.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

llvm="${LLVM_PREFIX:-/opt/homebrew/opt/llvm}"
if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
  export CC_wasm32_unknown_unknown="$llvm/bin/clang"
fi
if [ -z "${AR_wasm32_unknown_unknown:-}" ]; then
  export AR_wasm32_unknown_unknown="$llvm/bin/llvm-ar"
fi
for tool in "$CC_wasm32_unknown_unknown" "$AR_wasm32_unknown_unknown"; do
  if [ ! -x "$tool" ]; then
    echo "error: $tool not found. Install LLVM (brew install llvm) or set LLVM_PREFIX," >&2
    echo "       or CC_wasm32_unknown_unknown / AR_wasm32_unknown_unknown." >&2
    exit 1
  fi
done

if [ -z "${CFLAGS_wasm32_unknown_unknown:-}" ]; then
  manifest="$(cargo metadata --format-version 1 \
    | python3 -c 'import json,sys
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == "tree-sitter-language":
        print(p["manifest_path"]); break')"
  if [ -z "$manifest" ]; then
    echo "error: tree-sitter-language not found in cargo metadata" >&2
    exit 1
  fi
  inc="$(dirname "$manifest")/wasm/include"
  if [ ! -d "$inc" ]; then
    echo "error: $inc does not exist" >&2
    exit 1
  fi
  export CFLAGS_wasm32_unknown_unknown="-I$inc"
fi

if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  echo "error: run 'rustup target add wasm32-unknown-unknown'" >&2
  exit 1
fi
if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack not found (cargo install wasm-pack)" >&2
  exit 1
fi

wasm-pack build crates/wasm --release --target web --out-dir pkg
ls -l crates/wasm/pkg/*.wasm
