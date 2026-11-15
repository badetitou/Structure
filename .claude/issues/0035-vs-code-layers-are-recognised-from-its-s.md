---
id: 35
title: VS Code layers are recognised from its source layout
status: todo
parent: 20
branch: null
remote: 24
created: 2026-11-06
---
## Context

See #20. On vscode, TS rules give only 3 layers (`model`, `util`, `unknown`) and 1 974
domains: `rules/typescript.toml` expects `src/app/` (Angular). VS Code's architecture is
explicit in its paths: `src/vs/{base,platform,editor,workbench,code,server}` and, inside
each, environment folders `common`, `browser`, `node`, `electron-main`,
`electron-browser`/`electron-sandbox`, `worker`; `workbench/contrib/<feature>`,
`workbench/services/<service>`, `platform/<service>`.

## Acceptance criteria
- [ ] Layers from the `src/vs/<layer>` segment (base, platform, editor, workbench, …),
  declared in rules (toml), not hard-coded.
- [ ] Domains from the feature/service segment (`contrib/<x>`, `services/<x>`,
  `platform/<x>`), so the domain count is in the low hundreds with a domain tree.
- [ ] Existing goldens unchanged; a rule-level unit test on vscode-like paths.

## Out of scope
- Environment (`common/browser/node`) as a second axis — maybe a later view.
