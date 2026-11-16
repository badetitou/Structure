# Classification rules

Default rules per language: `java.toml`, `typescript.toml`, `pharo.toml`
(embedded in the binaries and the wasm build). A project can add
`<project>/.structure/rules.toml`, merged over the default rules of its
language.

## Layers

Each file declares the layers it uses, one `[[layer]]` entry per matrix row:

```toml
[[layer]]
id = "entry"       # used by rules (`layer = "entry"`)
label = "entry"    # text shown for the row
order = 3          # rows are sorted by order, top to bottom
```

All three fields are required. The `unknown` layer (unclassified symbols) is
implicit: it is never declared and is always the last row. A layer declared
twice in one file is an error.

The default files declare the same layers: `ui` (0), `state` (1),
`api_client` (2), `entry` (3), `business` (4), `persistence` (5), `model` (6),
`util` (7). `model` must be declared: it receives DTO-like classes that no
rule matches.

## Rules

```toml
[[rule]]
layer = "persistence"        # a declared layer, or `unknown`
kind = ["interface"]
name_suffix = ["Mapper"]
```

The first matching rule (file order) wins. A rule matches when all of its
matchers match; each matcher matches when any of its values does. Matchers:
`any_annotation`, `any_supertype`, `name_suffix`, `name_prefix`,
`name_contains`, `path_contains` (against `/<file>`), `package_contains`
(against `.<package>.`), `uses`, `kind`. A rule may carry its own `domain`
spec; the file-wide one is the `[domain]` table (see `DomainSpec` in
`crates/classify/src/classify.rs`).

A rule on a layer that is not declared is an error naming the rules file and
the layer, for example:

```text
rules/java.toml: invalid classification rules: rule 2 uses undeclared layer `service` (declared layers: ui, state, ...)
```

## Project rules

`.structure/rules.toml` is merged over the default rules:

- its rules come first (they take priority);
- its `[domain]` table, when present, replaces the default one;
- its layers are merged by `id`: an existing id takes the project's `label`
  and `order`, a new id is added.

The merged set is then checked: a project rule may use any layer declared by
the default rules or by the project file.
