# Structure (POC)

Analyzes a Java or TypeScript project (tree-sitter + SCIP index), classifies
classes into a layer x domain matrix, and shows the impact of a diff.

## How to try the POC

Prerequisites: Rust (stable), Node 20+, JDK + Maven (for Java indexing),
`cargo install tauri-cli --version "^2"`. Optional: `scip-java` or Coursier
(`cs`) for Java, `npx` for TypeScript (indexers are run automatically).

Fixtures and indexes (already present in `fixtures/` on this machine):

```
scripts/fetch-fixtures.sh      # clones jpetstore-6 and angular-realworld into fixtures/
scripts/index-fixtures.sh      # writes fixtures/*/index.scip
```

CLI:

```
cargo run -p structure-cli -- analyze fixtures/jpetstore-6 -o matrix.json
cargo run -p structure-cli -- analyze fixtures/angular-realworld --no-scip   # tree-sitter only
cargo run -p structure-cli -- impact fixtures/jpetstore-6 -o impact.json     # working tree vs HEAD
cargo run -p structure-cli -- impact <path> --base main --head feature
```

`analyze` prints language, step timings, link stats, warnings and a
layers x domains table on stderr; JSON goes to `-o` or stdout. An existing
`<project>/index.scip` is reused unless `--reindex`. Custom rules in
`<project>/.structure/rules.toml` take priority over the built-in ones.

App:

```
cd app/ui && npm install
cd ../src-tauri && cargo tauri dev
```

Pick a project folder, explore the matrix, then try impact: edit a file in the
fixture (e.g. `AccountMapper.java` in jpetstore-6), click "Show impact".

Tests: `STRUCTURE_FIXTURES=$PWD/fixtures cargo test` (e2e tests skip when a
fixture or its `index.scip` is missing; they never run indexers).

## Limitations

- Class-level granularity; matrix classification is heuristic (rules in `rules/*.toml`).
- Without `index.scip` (or if indexing fails) only tree-sitter edges are used,
  so call edges are far fewer.
- Impact follows reverse edges only; no dynamic dispatch/reflection/DI wiring beyond SCIP.
- Language detected from `pom.xml`/`build.gradle*` (Java) or `tsconfig.json`/`package.json` (TS); no mixed projects.
- Indexing can take minutes the first time (Maven build / npm install).
- scip-java runs the project's Maven build. A `~/.m2/settings.xml` pointing to
  an unreachable private repository makes it hang (the app gives up after
  10 min and falls back to tree-sitter). `scripts/index-fixtures.sh` avoids it
  for jpetstore through `.mvn/maven.config` (Central only, tests skipped).
