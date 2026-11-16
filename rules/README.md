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

A project can adapt the defaults with its own file,
`<project root>/.structure/rules.toml` (the root is the directory given to
`structure analyze` or opened in the app). It uses the same format as the
default files, every section optional. Without the file nothing changes.

It is merged over the default rules of the project's language:

- its rules come first (they take priority);
- its `[domain]` table, when present, replaces the default one;
- its layers are merged by `id`: an existing id takes the project's `label`
  and `order` (rename, reorder), a new id is added.

The merged set is then checked: a project rule may use any layer declared by
the default rules or by the project file.

Example: a `view` row above the default ones, the `entry` row renamed, and a
rule placing `*View` classes in `view`:

```toml
[[layer]]
id = "view"
label = "Views"
order = 0        # same order as `ui`: ties keep `ui` first

[[layer]]
id = "entry"
label = "Entry points"
order = 3

[[rule]]
layer = "view"
name_suffix = ["View"]
```

### Several projects

When several roots are analysed together (`structure analyze a b`, or several
folders in the app), each root is classified with its own defaults and its own
`.structure/rules.toml`, then the layers are merged by `id`:

- the first root (the primary one, whose name labels the matrix) keeps its
  layers: for a shared id, its `label` and `order` win;
- a layer declared only by later roots is added after the first root's
  layers, in root order, then in that root's order (its own `order` value is
  replaced to keep it after);
- `unknown` stays last.

The rows of the primary project therefore never move when a project is added.

### Errors

An invalid project file stops the analysis (the app shows it as an analysis
error); the message names the file and the reason, for example:

```text
invalid /path/to/app/.structure/rules.toml: invalid classification rules: TOML parse error at line 2, column 1 ... unknown field `name_sufix`, expected one of `layer`, `any_annotation`, ...
invalid /path/to/app/.structure/rules.toml: invalid classification rules: rule 1 uses undeclared layer `view` (declared layers: ui, ...)
invalid /path/to/app/.structure/rules.toml: invalid classification rules: layer `view` is declared twice
```

Errors cover TOML syntax, unknown fields, a layer declared twice in the file,
a declared `unknown` layer, and a rule on an undeclared layer.
