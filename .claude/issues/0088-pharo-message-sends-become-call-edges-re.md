---
id: 88
title: Pharo message sends become Call edges resolved by self/super, class receivers and unique selectors
status: todo
parent: 86
branch: null
remote: null
created: 2026-11-16
---
## Context

Second slice of #86. Builds on #87's crate. Smalltalk sends are untyped; the user
chose a precision-first heuristic: wrong edges are worse than missing ones.

## Acceptance criteria
- [ ] Message sends (unary, binary, keyword, cascades) are extracted from method
  bodies with their receiver kind.
- [ ] Resolution, in order:
  - `self foo` → `foo` looked up from the method's class up its project superclasses
    (then down to subclasses' overrides is NOT added);
  - `super foo` → lookup starting at the superclass;
  - `Foo bar` (class name receiver) → class-side `bar` of `Foo` or its supers;
    `Foo new`/`Foo new: …` also counts as a `Uses` of `Foo` (already from #87);
  - any other receiver → the implementor of that selector only when exactly one
    project class implements it (instance or class side counted separately);
  - otherwise no edge. Common Object protocol selectors (`=`, `printOn:`, `value`,
    `ifTrue:`…) are never resolved by the unique-implementor rule.
- [ ] `Call` edges carry the send's source range.
- [ ] Tests for each rule, including an ambiguous selector that yields no edge.
- [ ] Run stats (`metrics`) report resolved / unresolved sends for Pharo.
- [ ] fmt, clippy `-D warnings`, test green.

## Out of scope
- Type inference beyond the rules above; trait method resolution.
