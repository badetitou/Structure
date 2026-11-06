# Fixture scripts

Demo projects for the POC live in `fixtures/` (gitignored), or wherever
`STRUCTURE_FIXTURES` points.

1. `scripts/fetch-fixtures.sh` clones `mybatis/jpetstore-6` and
   `gothinkster/angular-realworld-example-app` at pinned commits, then builds them
   (`mvn package`, `npm install`).
2. `scripts/index-fixtures.sh` writes `index.scip` at each fixture root:
   - Java: `scip-java` via coursier (`cs`, installed into `~/.local/bin` if missing).
   - TypeScript: `npx @sourcegraph/scip-typescript`.
   Set `FORCE=1` to regenerate.

Prerequisites: git, JDK 17+ (21 tested), Maven, Node 20+, curl.

Known issue: a `~/.m2/settings.xml` that points to an unreachable private
repository makes Maven hang. Both scripts therefore run Maven with
`scripts/maven-settings.xml` (Maven Central only). `index-fixtures.sh` adds it to the
fixture's `.mvn/maven.config`, because `mvnw` ignores `MAVEN_ARGS`.

Goldens for tests: `tests/golden/{jpetstore,realworld}.toml`.
