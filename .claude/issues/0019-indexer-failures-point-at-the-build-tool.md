---
id: 19
title: Indexer failures point at the build tool that actually failed
status: done
parent: null
branch: 19-indexer-hint
remote: null
created: 2026-11-06
---
## Context

Analysing VerveineJ (Gradle wrapper 9.6.1, `foojay-resolver-convention` 0.8.0
in `settings.gradle.kts`) fails in 4 s: Gradle 9 removed
`JvmVendorSpec.IBM_SEMERU`, which foojay < 0.10 references. The project's own
`./gradlew build` fails the same way, so this isn't a Structure bug. But the
warning ends with "for Java also check ~/.m2/settings.xml"
(`crates/core/src/pipeline.rs:334`, timeout message at `:289`), which sends a
Gradle user to Maven config. The hint should name the build tool that ran
and the command that reproduces the failure.

## Acceptance criteria
- [ ] Indexer failure and timeout messages give a hint for the detected build:
      Maven (`pom.xml`) → check `~/.m2/settings.xml`, `mvn -q compile`;
      Gradle → run `./gradlew help` (or `gradle help` without a wrapper) in the
      project; TypeScript → no Java hint.
- [ ] Build detection reuses the marker files from `detect_language`
      (no new walk); a pure function chooses the hint and is unit-tested
      for the Maven, Gradle (with and without `gradlew`) and TS cases.
- [ ] The Gradle tail lines (`> ...` and `BUILD FAILED`) stay in the message
      as they are now.
- [ ] fmt, clippy `-D warnings` and tests pass.

## Out of scope
- Fixing VerveineJ's build (bumping foojay to ≥ 0.10 is up to the user).
- Choosing a Gradle version or JDK for the project, or retrying without
  toolchains.
- The "symbol positions come from the working tree" warning (#14).
