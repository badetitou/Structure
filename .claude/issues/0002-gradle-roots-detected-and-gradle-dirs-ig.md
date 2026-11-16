---
id: 2
title: Gradle roots detected and Gradle dirs ignored
status: done
parent: 1
branch: 2-gradle-detection
remote: 2
created: 2026-11-06
---
## Context

`crates/core/src/pipeline.rs` `detect_language` misses roots that only have
`settings.gradle(.kts)` (multi-project builds). `.gradle/` (Gradle cache) is not
excluded by the Java walker (`java::parse_project`) nor by `newer_sources`.

## Acceptance criteria
- [ ] `settings.gradle` and `settings.gradle.kts` alone → `Language::Java`; error message lists them.
- [ ] `.gradle/` is skipped by `java::parse_project` and `newer_sources`.
- [ ] Unit tests for each detection file and for `.gradle/` skipping (temp dirs).
- [ ] fmt, clippy -D warnings, tests green.

## Out of scope
- Fixtures, scripts, README (#3).
