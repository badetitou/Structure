---
id: 1
title: Structure analyses Gradle Java projects end to end
status: done
parent: null
branch: null
remote: 1
created: 2026-11-06
---
## Context

`detect_language` already treats `build.gradle`/`build.gradle.kts` as Java and the
Java walker skips `build/`, so a simple single-module Gradle project may already
work — but nothing verifies it. Gaps: a multi-project root with only
`settings.gradle(.kts)` is not detected, `.gradle/` is walked, scip-java's Gradle
path (wrapper, JDK compatibility, timeout) is untested, and docs only mention Maven.

## Acceptance criteria
- [ ] #2 done: Gradle roots detected, Gradle working dirs ignored.
- [ ] #3 done: a pinned Gradle fixture is fetched, indexed and checked by an e2e golden test.
- [ ] README prerequisites/limitations mention Gradle.

## Out of scope
- Kotlin/Groovy sources (only `.java` files are analysed).
- Per-subproject matrices; a multi-project build is analysed as one tree.
- Projects with both `pom.xml` and `build.gradle` (keep current behaviour).
