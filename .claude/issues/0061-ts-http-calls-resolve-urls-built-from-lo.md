---
id: 61
title: TS HTTP calls resolve URLs built from local variables
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

Real test, wegf backend + frontend (`--no-scip`): `http: 1417/2043 linked, 1 unknown
path, 10 ambiguous` (69%). 544 of the 615 unmatched calls come from the OpenAPI-generated
clients (`frontend/generated/wegf-open-api-*/api/*.service.ts`):

    let localVarPath = `/habilitations/hasHabilitationNiveauApAe`;
    return this.httpClient.request<boolean>('get', `${this.configuration.basePath}${localVarPath}`, ...

`${...}${localVarPath}` becomes `/{basePath}{localVarPath}`, one segment, so it never matches.
Another 68 call `{apiComptaUrl}/Dette/...`, `/UcCollectivite/...` (the `casino` services): the
backend has no such endpoint, so they are correctly unmatched.

## Acceptance criteria
- [ ] In a template/concatenated URL, an identifier substitution resolves to the string/template
  initialiser of a `const`/`let` in the same function (or a readonly class field), if that variable
  is never reassigned; resolution is one level deep, otherwise placeholder as today.
- [ ] Adjacent placeholders with no `/` between them (`${a}${b}`) no longer fuse into one segment
  when `b` resolves to a path starting with `/`.
- [ ] Unit tests for the OpenAPI generator shape (with `encodeParam` path params), reassignment → placeholder.
- [ ] wegf backend + frontend: linked ratio reported before/after, target >= 80% of calls whose
  endpoint exists in the backend (casino calls excluded and counted separately); angular-realworld
  test stays exact (18 calls).

## Out of scope
- Cross-function data flow, environment files, interceptors.
