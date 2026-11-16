---
id: 41
title: Languages plug in through a frontend trait instead of match arms
status: done
parent: 36
branch: 41-language-frontend
remote: 30
created: 2026-11-06
---
## Context

Each language is dispatched by `match Language::` across classify.rs (35),
pipeline.rs (15), link, ir, matrix, metrics, impact. Introduce a
`LanguageFrontend` trait (detect, indexer command/args, source extensions,
parse to `Graph`, default rules file, language-specific classify hooks) with
`java` and `ts` implementations and a registry, so a new language is one
module plus one registry line.

## Acceptance criteria
- [ ] Outside the frontends and the registry, no `match Language::` remains
      (or each remaining one is justified in the report).
- [ ] Golden classifications (`tests/golden/`) and e2e tests unchanged.
- [ ] fmt, clippy, test pass.

## Out of scope
- Adding a language; crate split (#42).
