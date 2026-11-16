---
id: 23
title: SCIP load and linking keep memory proportional to the graph
status: done
parent: 20
branch: 23-link-memory
remote: 20
created: 2026-11-06
---
## Context

See #20, scout findings: `scip.rs:79-101` (whole file read + full decode + copies),
`link.rs:173-231` (two Strings per edge, `ext:` edges for every external ref),
`link.rs:294-296` (global sort/dedupe), `link.rs:262-291` (unmemoized implementer DFS),
`link.rs:136-143`, `184-192` (linear scans), `ts.rs:683-692` (string clones in dedupe).

## Acceptance criteria
- [ ] Measured with #21 on vscode: meets #20's target, or the report says exactly what blocks it.
- [ ] Edges deduped as they are produced; implementer closure memoized per target.
- [ ] SCIP occurrences not held as owned Strings per occurrence (intern or stream per document).
- [ ] Graph output identical on the fixtures (goldens and link mapping ratio unchanged).

## Out of scope
- Changing the `Graph` JSON contract seen by the UI.
