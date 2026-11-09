---
id: 58
title: HTTP edges link frontend calls to backend endpoints
status: todo
parent: 54
branch: null
remote: null
created: 2026-11-09
---
## Context

Needs #55 (merged graph), #56 (endpoints), #57 (calls).

## Acceptance criteria
- [ ] New `EdgeKind::Http` from the TS caller symbol to the Java handler method.
- [ ] Matching: same verb; paths compared segment by segment, placeholders match
  any segment; a common prefix on one side (e.g. `/api`, a base URL) is tolerated
  by suffix matching; ambiguous matches choose the most specific one, ties produce no edge.
- [ ] Matrix aggregates Http edges like other kinds (cell edges count by kind),
  so front `api_client` cells link to back `entry` cells.
- [ ] Run metrics report calls found / linked / unmatched.
- [ ] realworld pair: ≥ 80% of the Angular service calls linked; unmatched ones listed.

## Out of scope
- UI styling of Http edges beyond what the matrix already shows per kind.
