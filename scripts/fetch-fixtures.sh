#!/usr/bin/env bash
# Clone demo projects at pinned commits and build them. Idempotent.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="${STRUCTURE_FIXTURES:-$ROOT/fixtures}"
mkdir -p "$FIXTURES"

JPETSTORE_URL=https://github.com/mybatis/jpetstore-6
JPETSTORE_SHA=8f11fab492a473d9f98a4eb71de0af8d565f38fe
REALWORLD_URL=https://github.com/gothinkster/angular-realworld-example-app
REALWORLD_SHA=d426ec804f7d035c374aa4a1b48ecdb05a03eb02
SBRW_URL=https://github.com/gothinkster/spring-boot-realworld-example-app
SBRW_SHA=ee17e31aafe733d98c4853c8b9a74d7f2f6c924a

fetch() { # url dir sha
  if [ ! -d "$2/.git" ]; then git clone -q "$1" "$2"; fi
  if [ "$(git -C "$2" rev-parse HEAD)" != "$3" ]; then
    git -C "$2" checkout -q "$3"
  fi
  echo "$2 @ $3"
}

fetch "$JPETSTORE_URL" "$FIXTURES/jpetstore-6" "$JPETSTORE_SHA"
fetch "$REALWORLD_URL" "$FIXTURES/angular-realworld" "$REALWORLD_SHA"
fetch "$SBRW_URL" "$FIXTURES/spring-boot-realworld-example-app" "$SBRW_SHA"

if [ ! -d "$FIXTURES/jpetstore-6/target/classes" ]; then
  (cd "$FIXTURES/jpetstore-6" && mvn -q -s "$ROOT/scripts/maven-settings.xml" -DskipTests package)
fi
if [ ! -d "$FIXTURES/angular-realworld/node_modules" ]; then
  (cd "$FIXTURES/angular-realworld" && if [ -f package-lock.json ]; then npm ci; else npm install --no-package-lock; fi)
fi
echo "fixtures ready in $FIXTURES"
