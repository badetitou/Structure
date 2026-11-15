---
id: 8
title: Rules classify Spring/DGS domain and core packages in realworld-java
status: done
parent: 1
branch: 8-realworld-rules
remote: 8
created: 2026-11-06
---
## Context

On the Gradle fixture `spring-boot-realworld-example-app` (#3), tree-sitter-only analysis
gets layer 16/16 on the golden set, but 5 hand-checked classes were left out because
the current rules give them layer `unknown`, and domain accuracy is 6/16:
- `core.article.Article`, `core.user.User` (plain domain classes in a `core` package) → expected model.
- `core.article.ArticleRepository` (repository interface, no annotation) → expected persistence.
- `graphql.ArticleDatafetcher`, `graphql.UserMutation` (`@DgsComponent`) → expected entry; only `Component` is matched.
- `name_root` domain strategy yields `articlequery`, `commentsapi`, `mybatisarticle`, `defaultjwt`
  instead of `article`, `comment`, `article`, `jwt`.

## Acceptance criteria
- [ ] The five classes above are classified as expected and added back to `tests/golden/realworld-java.toml`.
- [ ] Domain extraction strips common role prefixes/suffixes (Query, Api, MyBatis, Default, …) or uses the package; domain accuracy on realworld-java improves without regressing jpetstore.
- [ ] e2e tests for jpetstore and realworld-java pass.

## Out of scope
- Asserting domain accuracy in the e2e harness (separate decision).
