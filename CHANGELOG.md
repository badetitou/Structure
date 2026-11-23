# Changelog

All notable changes to Structure are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). Pushing a `v*` tag builds the
browser extension and attaches it to the GitHub release.

## [Unreleased]

## [0.2.2] - 2026-11-23

### Changed

- Updated Model to the crates.io release `0.5.0`: `model-core`, `model-scip`
  and `model-link` move from `0.3` to `0.5`. Model 0.4 adds relation kinds
  (`DeclaredType`, `Accesses`, `Throws`, `Overrides`) and new crates; Structure
  uses none of the breaking GraphQL or index-API changes, and its build and
  tests pass unchanged.

## [0.2.1] - 2026-11-19

### Changed

- Switched Model to the crates.io release `0.3`: Structure now depends on
  `model-core`, `model-scip` and `model-link` directly, instead of the git
  `model` facade. Its breaking GraphQL refactor does not affect Structure,
  which only uses the SCIP reader/linker and the index pipeline.

## [0.2.0] - 2026-11-17

Re-architecture for stability (#68): languages, layers, views and highlight
sources are pluggable. Target and remaining gaps: `docs/architecture.md`.

### Added

- **Configurable layers**: rules files declare their layers (`[[layer]]` with
  `id`, `label`, `order`); a project's `.structure/rules.toml` can add layers
  or relabel and reorder the built-in ones. A rule on an undeclared layer is an
  error naming the file and the layer. Editing the rules file reloads the
  matrix without re-indexing or re-parsing (automatic, or "Reload rules" in
  the options); an invalid file keeps the current matrix and shows the error.
- **Highlight overlays**: impacts are kept as named overlays with their own
  colour. "Keep" pins the current impact; a selector above the matrix (shown
  from two overlays) selects one, and cmd-click (macOS) / ctrl-click selects
  several, drawn together as colour bands per cell.
- **Several matrix views**: Views → "New matrix view" opens another matrix;
  each matrix panel keeps its own display options (mode, transpose, align,
  HTTP/RPC links) and its own lens: `layer × domain` (inferred domains, the
  default) or `layer × package` (raw packages / directories).
- **Repositories with several languages**: a root holding e.g. a Java backend
  and a TypeScript frontend is analysed as one project per language
  (`<dir> (java)`, `<dir> (typescript)`, with `index.<language>.scip`) and
  merged, so HTTP calls between them are linked. The browser extension does
  the same for a pull request.
- CI on every pull request: Rust (fmt, clippy, tests) and UI (vitest,
  svelte-check).

### Changed

- A multi-project analysis merges the roots' layers by id: the first root's
  labels and order win, and layers only a later root declares are appended
  after them (before `unknown`), so a later root no longer overrides a label.
- The default rules moved to `crates/lang/rules/` (error messages still name
  `rules/<lang>.toml`).
- A Java repository with a `package.json` and real `.ts`/`.tsx` sources is now
  analysed as two projects (Java only before).
- Internals: a `Session` in `structure-core` owns the analysis cache (the Tauri
  commands are thin wrappers); the UI wire types are generated from Rust
  (`app/ui/src/generated/`); a pure `structure-lang` registry is shared by the
  desktop and wasm builds; an optional per-language `Indexer`; HTTP and GWT RPC
  links go through protocol ports and linkers; a pure `structure-highlight`
  crate computes impacts; the UI state is split into stores and views go
  through a `ViewContext`.

### Fixed

- A commit impact of a multi-language project at the repository root no longer
  names the snapshot after the commit SHA (its cells now match the working
  tree).
- Impact finds the changed symbols when a folder expands to a single
  sub-project, and a commit impact on such a folder no longer fails with
  "cannot detect language".
- A working-tree impact with invalid project rules keeps the cached analysis.
- Lens panels keep their zoom and open groups while a lens loads, show and
  retry overlays they cannot map, and no longer drop a selection made in
  another panel.

## [0.1.3] - 2026-11-16

### Fixed

- The release workflow gives Firefox Add-ons the listing category
  (Web Development), which it requires for the first listed version.

## [0.1.2] - 2026-11-16

### Added

- Structure is released under the MIT license (`LICENSE`).

### Fixed

- The release workflow sends the license to Firefox Add-ons, which rejected
  the 0.1.1 submission without one.

## [0.1.1] - 2026-11-16

### Changed

- The Structure panel of the browser extension uses GitHub's panel padding.
- The release workflow uses the Node 24 versions of the GitHub actions.

## [0.1.0] - 2026-11-16

First public release: a proof of concept, local only.

### Added

- **Analysis pipeline**: tree-sitter frontends for Java, TypeScript and Pharo
  (Tonel `.st`) extract classes, ranges, annotations and supertypes; SCIP
  indexes (`scip-java`, `scip-typescript`) add compiler-accurate `Call`,
  `Uses` and `Implements` edges. An existing `index.scip` is reused; without
  one, Structure falls back to tree-sitter edges.
- **Classification**: TOML rules assign each class a layer and a domain, with
  built-in rules for Spring, Stripes, MyBatis, JPA, Angular, NestJS and Pharo
  packages, and per-project overrides in `.structure/rules.toml`. Domains come
  from packages and modules, including package-by-feature layouts.
- **Layer × domain matrix**: symbols fold into cells, edges are aggregated
  between cells.
- **Impact**: a git diff (working tree or `base..head`) maps to changed cells
  and to the cells that depend on them, by distance.
- **Multi-root analysis**: several project folders in one matrix. TypeScript
  HTTP calls are linked to Spring and JAX-RS endpoints (including mappings
  inherited from interfaces and generated OpenAPI sources); GWT RPC calls are
  linked to their service implementations.
- **Pharo**: message sends become `Call` edges, resolved through `self`/`super`,
  class receivers and unique selectors.
- **CLI** (`structure analyze`, `structure impact`): JSON output, per-phase
  timings, counts and peak memory.
- **Desktop app** (Tauri 2, Svelte 5): dockable panels, zoom and pan, hover
  tracing, Normal / Dense / Flame views, swapped axes, git history with
  per-commit impact, a Diff panel, and front/back domains aligned on shared
  rows.
- **Browser extension** (Chrome, Edge, Firefox): a *Structure* tab on GitHub
  pull requests that shows the PR's matrix and impact, computed in the browser
  with WebAssembly. Works for Java, TypeScript and Pharo repositories.
- **Release workflow**: a version tag builds the extension, attaches it to the
  GitHub release and, when the store secrets are set, submits it to Firefox
  Add-ons and the Chrome Web Store.

[Unreleased]: https://github.com/badetitou/Structure/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/badetitou/Structure/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/badetitou/Structure/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/badetitou/Structure/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/badetitou/Structure/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/badetitou/Structure/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/badetitou/Structure/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/badetitou/Structure/releases/tag/v0.1.0
