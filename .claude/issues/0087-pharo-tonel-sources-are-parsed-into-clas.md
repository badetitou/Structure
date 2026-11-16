---
id: 87
title: Pharo Tonel sources are parsed into classes, methods, inheritance and class references
status: done
parent: 86
branch: 87-pharo-tonel-parsing
remote: null
created: 2026-11-16
---
## Context

First slice of #86: a `Language::Pharo` variant and a `structure-lang-pharo` crate
that reads Tonel sources. The steps for a new language are listed at the top of
`crates/core/src/lang/mod.rs`.

## Acceptance criteria
- [x] `Language::Pharo` (serde `pharo`) in `structure-ir`; `app/ui/src/api.ts`
  `Language` type and `docs/api.md` updated.
- [x] New crate `crates/lang-pharo` with a hand-written Tonel reader (no new
  dependency: no maintained tree-sitter grammar for Pharo on crates.io; Tonel is a
  STON header + `Class >> selector [ body ]` blocks). It handles strings, comments,
  symbols, character literals and nested brackets when finding method bodies, and
  both Tonel v1 (`#name : 'Foo'`) and v3 (`#name : #Foo`) headers.
- [x] Symbols: one per class (with package as module path) and per method, instance
  and class side (`Foo class >> bar`), extension methods (`Extension { #name : #Foo }`)
  attached to their package; source ranges set.
- [x] Edges: superclass (`ext:<Name>` when not in the project); `Uses` for class
  names referenced in method bodies (capitalised identifiers that are project or
  global class names; not instance/temp variables).
- [x] `PharoFrontend`: detect `src/.properties` or any `*.class.st` under `src/`;
  extensions `st`; `indexer_command` → `None`; module separator `-` (package names
  like `MyApp-Core-Model`); `rules/pharo.toml` with sensible layers (`*-Tests` → tests,
  `*-UI`/`*-Spec`/`*-Widgets` → UI, `*-Model`/`*-Core` → domain, …).
- [x] Unit tests on inline Tonel samples (v1 and v3, class side, extension, trait
  usage in header ignored gracefully); `registry_is_consistent` passes.
- [x] fmt, clippy `-D warnings`, test green.

## Out of scope
- Message send → `Call` edges (#88).
