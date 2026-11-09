---
id: 62
title: A root without a build file is analysed as its sub-projects
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

`structure analyze /…/WeMagnus/wegf` fails: "cannot detect language" — the root holds
`backend/` (Maven), `frontend/` (Nx, package.json) and `we-elements/` (TS) but no build
file itself. The user had to pass `wegf/backend wegf/frontend` by hand.

## Acceptance criteria
- [ ] A root with no build file of its own whose direct children (depth 1, then 2 if none)
  are detectable projects is analysed as those sub-projects, exactly as if they were passed
  as several roots (#55), names = child dir names.
- [ ] A root that is itself detectable keeps today's behaviour (no splitting), so Maven/Gradle
  multi-module and Nx roots are unchanged.
- [ ] CLI and app both benefit (pipeline level); the CLI prints the detected projects.
- [ ] Test on a temp dir layout; wegf root gives backend + frontend + we-elements and the same
  http stats as passing backend + frontend (+ we-elements).

## Out of scope
- Mixed roots that are themselves a project and also contain another language's project
  (e.g. a Maven root with a `frontend/` package.json inside) — note as follow-up.
