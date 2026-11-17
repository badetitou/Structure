#!/usr/bin/env bash
# Run every check a change must pass, with short output: one line per check,
# and for a failing check only the tail of its log (full log kept in a temp
# file). Exit status is non-zero when any check fails.
#
#   scripts/verify.sh            all checks
#   scripts/verify.sh rust ui    only the named checks
#
# Checks: fmt, clippy, test (Rust workspace, fixtures from STRUCTURE_FIXTURES,
# default <workspace>/fixtures), tauri (fmt + clippy of app/src-tauri),
# ui (vitest + svelte-check in app/ui), wasm (scripts/build-wasm.sh).
# Groups: rust = fmt clippy test.
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export STRUCTURE_FIXTURES="${STRUCTURE_FIXTURES:-$root/fixtures}"
TAIL="${VERIFY_TAIL:-40}"

all=(fmt clippy test tauri ui wasm)
want=()
for a in "$@"; do
  case "$a" in
    rust) want+=(fmt clippy test) ;;
    fmt | clippy | test | tauri | ui | wasm) want+=("$a") ;;
    *) echo "unknown check: $a (expected: ${all[*]} rust)" >&2; exit 2 ;;
  esac
done
[ ${#want[@]} -eq 0 ] && want=("${all[@]}")

tmp="${TMPDIR:-/tmp}"
logs="$(mktemp -d "${tmp%/}/structure-verify.XXXXXX")"
failed=0

run() {
  local name="$1"; shift
  local log="$logs/$name.log" start=$SECONDS
  if "$@" >"$log" 2>&1; then
    echo "ok   $name ($((SECONDS - start))s)"
  else
    failed=1
    echo "FAIL $name ($((SECONDS - start))s) — full log: $log"
    # Errors first when the tool marks them, else the tail.
    if grep -qE '^(error|warning: unused|test .* FAILED|failures:|---- )|FAIL |✗|Error:' "$log"; then
      grep -nE '^(error|test .* FAILED|failures:|---- )|FAIL |✗|Error:|panicked at' "$log" | head -n "$TAIL"
    else
      tail -n "$TAIL" "$log"
    fi
  fi
}

for c in "${want[@]}"; do
  case "$c" in
    fmt) run fmt cargo fmt --check ;;
    clippy) run clippy cargo clippy --workspace --all-targets -- -D warnings ;;
    test) run test cargo test --workspace ;;
    tauri) run tauri bash -c 'cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings' ;;
    ui) run ui bash -c 'cd app/ui && npx vitest run && npm run check' ;;
    wasm) run wasm scripts/build-wasm.sh ;;
  esac
done

[ $failed -eq 0 ] && rm -rf "$logs"
exit $failed
