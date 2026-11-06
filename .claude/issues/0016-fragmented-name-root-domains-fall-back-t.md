---
id: 16
title: Fragmented name-root domains fall back to package domains with a domain tree
status: done
parent: 15
branch: 16-module-domains
remote: null
created: 2026-11-06
---
## Context

See #15. `classify::domain_of` uses the rule set's `[domain]` spec per symbol; with
`name_root`, VerveineJ gets 230 singleton domains (97% of classes alone in their
domain). Fixtures today: jpetstore 8%, angular-realworld 13%, realworld-java 54% of
classes in singleton domains.

## Acceptance criteria
- [ ] New `Strategy::Auto` (default of the builtin java/ts rules): name-root domains,
  unless ≥ 75% of the classes using the default spec end up alone in their domain;
  then module domains: the package (Java) or the file's directory (TS).
- [ ] Module domain = shortest suffix of the module path (in segments) that is unique
  among the project's modules, lower-cased (`famixtraits`, `refvisitors`, `utils`).
- [ ] `Matrix.domain_tree?: DomainNode[]` (omitted when absent): module trie with
  single-child chains without own classes collapsed (`org.moosetechnology.model.famix`
  is one node). `DomainNode { label, domain?, size, children }`; a module with own
  classes and sub-modules gets a leaf child for its own classes. Every matrix domain
  is exactly one leaf.
- [ ] An explicit `strategy = "name_root"` (user rules) never switches.
- [ ] VerveineJ (`--no-scip`) gives ≤ 15 domains; jpetstore, angular-realworld and
  realworld-java classifications unchanged (golden e2e green).
- [ ] `docs/api.md` and `docs/samples` updated (UI `api.ts` types are #17); unit tests.

## Out of scope
- UI collapse/expand (#17). Layer rules.
