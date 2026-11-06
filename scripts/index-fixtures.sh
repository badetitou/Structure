#!/usr/bin/env bash
# Generate index.scip at the root of each fixture.
# Idempotent: skips existing indexes (FORCE=1 to regenerate).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="${STRUCTURE_FIXTURES:-$ROOT/fixtures}"
SCIP_JAVA_VERSION=0.12.3
SCIP_TS_VERSION=0.6.1
export PATH="$HOME/.local/bin:$PATH"
# Avoid user-level ~/.m2/settings.xml private repos (may be unreachable and stall the build)
export MAVEN_ARGS="${MAVEN_ARGS:--s $ROOT/scripts/maven-settings.xml}"

if ! command -v cs >/dev/null; then
  mkdir -p "$HOME/.local/bin"
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) url=https://github.com/coursier/launchers/raw/master/cs-aarch64-apple-darwin.gz ;;
    Darwin-x86_64) url=https://github.com/coursier/launchers/raw/master/cs-x86_64-apple-darwin.gz ;;
    Linux-x86_64) url=https://github.com/coursier/launchers/raw/master/cs-x86_64-pc-linux.gz ;;
    *) echo "unsupported platform" >&2; exit 1 ;;
  esac
  curl -fsSL "$url" | gzip -d > "$HOME/.local/bin/cs"
  chmod +x "$HOME/.local/bin/cs"
fi

report() { echo "$1: $(du -h "$2" | cut -f1) ($2)"; }

J="$FIXTURES/jpetstore-6"
if [ "${FORCE:-0}" = 1 ] || [ ! -f "$J/index.scip" ]; then
  (cd "$J" && MAVEN_ARGS="-s $ROOT/scripts/maven-settings.xml -B" cs launch "com.sourcegraph:scip-java_2.13:$SCIP_JAVA_VERSION" -- index --output index.scip)
fi
report java "$J/index.scip"

T="$FIXTURES/angular-realworld"
if [ "${FORCE:-0}" = 1 ] || [ ! -f "$T/index.scip" ]; then
  (cd "$T" && npx -y "@sourcegraph/scip-typescript@$SCIP_TS_VERSION" index --output index.scip)
fi
report typescript "$T/index.scip"
