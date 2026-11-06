---
id: 15
title: Projects without a layered architecture still get a readable structure
status: done
parent: null
branch: null
remote: null
created: 2026-11-06
---
## Context

On a tool/library like VerveineJ (`fr.inria.verveine.extractor.java` + the generated
FAMIX metamodel `org.moosetechnology.model.famix.*`, 235 files), tree-sitter analysis
gives 2 layers (`model`, `unknown`) and **230 domains of one class each**: the
`name_root` domain strategy only makes sense when a name root recurs across layers
(`Account`Controller/Service/Mapper). The matrix is 230 columns wide and unusable.
The model side is dominated by one package (`famixtraits`, 129 traits) next to
`famixjavaentities` (57) and small ones.

## Acceptance criteria
- [ ] #16: when name-root domains are fragmented, domains come from packages (Java) /
  directories (TS), and the matrix carries the package tree.
- [ ] #17: the UI collapses that tree so the first view fits ~12 columns, and the user
  can expand/collapse a group to dig in.

## Out of scope
- Splitting one big package further (dependency clustering, name-stem groups): follow-up.
- New layer inference for non-layered code (e.g. `main` → entry): follow-up.
