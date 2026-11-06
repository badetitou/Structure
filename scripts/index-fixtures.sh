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
  # mvnw ignores MAVEN_ARGS; .mvn/maven.config is read by every Maven launcher.
  mkdir -p "$J/.mvn"
  grep -q -- "--settings=" "$J/.mvn/maven.config" 2>/dev/null ||
    echo "--settings=$ROOT/scripts/maven-settings.xml" >> "$J/.mvn/maven.config"
  grep -q -- "-DskipITs" "$J/.mvn/maven.config" || printf -- "-DskipTests=true\n-DskipITs=true\n" >> "$J/.mvn/maven.config"
  (cd "$J" && cs launch "com.sourcegraph:scip-java_2.13:$SCIP_JAVA_VERSION" -- index --output index.scip)
fi
report java "$J/index.scip"

# Gradle 7.4 wrapper cannot run on JDK 21: needs a JDK 17 (JAVA17_HOME, brew openjdk@17, java_home).
G="$FIXTURES/spring-boot-realworld-example-app"
if [ -d "$G" ]; then
  J17=""
  BREW17=""
  if command -v brew >/dev/null; then
    BREW17="$(brew --prefix openjdk@17 2>/dev/null || true)"
    [ -n "$BREW17" ] && BREW17="$BREW17/libexec/openjdk.jdk/Contents/Home"
  fi
  for cand in "${JAVA17_HOME:-}" "$BREW17" "$(/usr/libexec/java_home -v 17 2>/dev/null || true)"; do
    # java_home -v 17 may fall back to a newer JDK: always check the real version.
    if [ -n "$cand" ] && [ -x "$cand/bin/java" ]; then
      v=$("$cand/bin/java" -version 2>&1 || true)
      if printf '%s' "$v" | grep -Eq 'version "17[."]'; then J17="$cand"; break; fi
    fi
  done
  if [ "${FORCE:-0}" = 1 ] || [ ! -f "$G/index.scip" ]; then
    if [ -n "$J17" ]; then
      # Index to a temp file; a failure must not abort the other fixtures or leave a partial index.
      if (cd "$G" && JAVA_HOME="$J17" cs launch "com.sourcegraph:scip-java_2.13:$SCIP_JAVA_VERSION" -- index --output index.scip.tmp); then
        mv "$G/index.scip.tmp" "$G/index.scip"
      else
        rm -f "$G/index.scip.tmp"
        echo "warning: spring-boot-realworld-example-app: indexing failed" >&2
      fi
    else
      echo "skipping spring-boot-realworld-example-app: JDK 17 not found (set JAVA17_HOME)" >&2
    fi
  fi
  if [ -f "$G/index.scip" ]; then report java-gradle "$G/index.scip"; fi
fi

T="$FIXTURES/angular-realworld"
if [ "${FORCE:-0}" = 1 ] || [ ! -f "$T/index.scip" ]; then
  (cd "$T" && npx -y "@sourcegraph/scip-typescript@$SCIP_TS_VERSION" index --output index.scip)
fi
report typescript "$T/index.scip"
