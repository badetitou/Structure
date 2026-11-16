# Changelog

All notable changes to Structure are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). Pushing a `v*` tag builds the
browser extension and attaches it to the GitHub release.

## [Unreleased]

### Added

- Rules files declare their layers (`[[layer]]` with `id`, `label`, `order`);
  a project's `.structure/rules.toml` can add layers or relabel and reorder the
  built-in ones. A rule on an undeclared layer is an error naming the file and
  the layer.

### Changed

- A multi-project analysis merges the roots' layers by id: the first root's
  labels and order win, and layers only a later root declares are appended
  after them (before `unknown`), so a later root no longer overrides a label.

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

[Unreleased]: https://github.com/badetitou/Structure/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/badetitou/Structure/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/badetitou/Structure/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/badetitou/Structure/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/badetitou/Structure/releases/tag/v0.1.0
