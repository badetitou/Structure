---
id: 3
title: Gradle fixture indexed and covered by an e2e golden test
status: done
parent: 1
branch: 3-gradle-fixture
remote: 3
created: 2026-11-06
---
## Context

No Gradle fixture exists, so the scip-java Gradle path is never exercised.
Local JDKs: 21, 27 and 8. Gradle < 8.5 does not run on JDK 21, so the fixture
must ship a wrapper >= 8.5, or the script must point `JAVA_HOME` at a supported JDK.

## Acceptance criteria
- [ ] `scripts/fetch-fixtures.sh` clones a small, layered, Gradle-only Java project at a pinned SHA.
- [ ] `scripts/index-fixtures.sh` produces its `index.scip` (offline-safe like the Maven settings trick where needed).
- [ ] `tests/golden/<name>.toml` + an `e2e_pipeline.rs` case; skips when fixture/index absent.
- [ ] Link mapping ratio on the fixture is reported and comparable to jpetstore.
- [ ] README: prerequisites (JDK + Maven or Gradle wrapper), Gradle notes in Limitations.

## Out of scope
- Changes to detection/walker (#2). Kotlin sources.
