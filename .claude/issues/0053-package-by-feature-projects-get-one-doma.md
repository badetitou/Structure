---
id: 53
title: Package-by-feature projects get one domain per feature package
status: done
parent: null
branch: claude/auto-domain-grouping-7d1fda
remote: 41
created: 2026-11-09
---
## Context

On wegf/backend (WeMagnus, ~880 main classes) the `auto` domain strategy gives
636 domains. The code is package-by-feature: `wegf.<feature>.{controller,
mapper,service}`, `wegf.ecompta.<uc>.transverse`; class names do not share a
root (`UcTvaControleur`, `UcTvaOpenApiConfiguration`, `BudgetMcpTool`), so
name roots fragment, but not enough (< 75% alone) to switch to module domains,
and module domains would split each feature by layer anyway.

## Acceptance criteria
- [x] `auto` detects role segments: package (TS: directory) last segments found
  under >= 3 distinct parents (`controller`, `service`, `mapper`, `transverse`...).
- [x] Feature module = module minus trailing role segments, unless it is a
  container (ancestor of >= 2 other feature modules, e.g. `wegf` itself).
- [x] Feature mode turns on only when there are at least 8 feature modules and
  at least twice as many as role segments (so layer-then-feature code like realworld-java,
  where `article`/`user` recur under a few layers, is unchanged).
- [x] In feature mode, a class in a feature gets the feature domain; a class
  elsewhere (e.g. `wegf.entity.Apcp`, `wegf.repository.ApcpRepository`) joins
  a feature whose name matches its name root (ignoring case, `_`, plural s/x),
  else its own module; names and `domain_tree` as for module domains (#16).
- [x] wegf/backend: domains follow features (activites, biens, engagements,
  uctva...), entity/repository classes joined to their feature where named alike.
- [x] jpetstore, angular-realworld, realworld-java goldens unchanged; unit tests.

## Known limits
- A non-role segment under a role (`billing.service.foo`) is not stripped.
- A feature named like a role elsewhere is swallowed into its parent.
- Layer-then-feature code with >= 8 layer packages over few recurring
  entities would still get layer-named domains.

## Out of scope
- UI to regroup domains by hand (user rules `.structure/rules.toml` aliases /
  `package_segment` already allow it).
- Merging features by dependency clustering.
