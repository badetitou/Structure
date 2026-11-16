<p align="center">
  <img src="docs/images/logo.svg" width="96" alt="Structure logo">
</p>

<h1 align="center">Structure</h1>

<p align="center">
  <b>See a whole codebase as one matrix, and see what a diff really touches.</b><br>
  Structure turns a Java, TypeScript or Pharo project into a call graph, folds it into
  <code>layer × domain</code> cells, and renders it as an interactive matrix.
</p>

<p align="center">
  <img src="docs/images/matrix-hover.png" alt="The jpetstore-6 matrix: hovering business/catalog lights up what it reaches (green) and what reaches it (blue)">
</p>

<p align="center"><sub>
  <a href="https://github.com/mybatis/jpetstore-6">jpetstore-6</a> in Structure. Hovering
  <code>business/catalog</code> lights up what it reaches (green) and what reaches it (blue).
</sub></p>

> **Status: proof of concept.** Local only, Java, TypeScript and Pharo,
> class-level granularity. See [Limitations](#limitations) and the
> [changelog](CHANGELOG.md). Latest release:
> [GitHub releases](https://github.com/badetitou/Structure/releases/latest).

## Why

A call graph with thousands of symbols is unreadable. Structure does not draw
it symbol by symbol. It groups every class into a cell:

- a **layer** (row): `entry`, `business`, `persistence`, `model`, `util`, plus `ui` and `api_client` for front ends;
- a **domain** (column): `account`, `order`, `catalog`, and so on, derived from packages and modules.

Edges between symbols become edges between cells. You get a picture that fits
on one screen, reads top to bottom, and answers two questions quickly:

1. **How is this software organised?** Which domains exist, which layers they span, who calls whom.
2. **What does this change impact?** Which cells a diff modifies, and which ones depend on them.

## Features

| | |
| --- | --- |
| **Layer × domain matrix** | Cells sized by symbol count, edges aggregated between cells. Zoom, pan, expand domain groups. |
| **Hover to trace** | Forward and backward traversal from any cell: what it reaches, what reaches it. |
| **Diff impact** | Changed cells in red, transitive callers shaded by distance. Works on the working tree or any `base..head`. |
| **Git history** | Pick a commit in the history panel to see its matrix and its impact. |
| **Front + back together** | Analyse an Angular front and a Spring back in one matrix; HTTP calls are matched to endpoints. GWT RPC calls are linked to their service implementations. |
| **Three views** | *Normal* (labelled cells and edges), *Dense* (uniform tiles for big projects), *Flame* (column widths by size). |
| **Compiler-accurate edges** | Tree-sitter for symbols, [SCIP](https://github.com/sourcegraph/scip) indexes for exact references. |
| **Pharo too** | Tonel (`.st`) sources are read directly; message sends become call edges, resolved through `self`/`super`, class receivers and unique selectors. |
| **Custom rules** | Classification is plain TOML; override it per project. |
| **CLI and desktop app** | The `structure` binary emits JSON; the Tauri app is the interactive view. |
| **Browser extension** | A *Structure* tab on GitHub pull requests, in Chrome and Firefox: the PR's matrix and impact, computed in the browser (WebAssembly). |

## Tour

### Impact of a change

Edit `AccountMapper.java`, pick *Working tree* in the history panel. The
changed cell turns red; its callers are shaded by distance. The details panel
lists the changed and impacted symbols.

![Impact of an edit to AccountMapper: persistence/account is changed, business/account, entry/account and entry/order are impacted](docs/images/impact.png)

The Diff panel shows the matching textual diff next to the matrix.

![The Diff panel next to the impact matrix](docs/images/diff.png)

### Front and back in one matrix

Open several folders at once. Each project gets its own layer band, split by a
double line. A front domain and the back domain it calls share a column
(`angular-realworld/article ⇄ spring-boot…/article`).

![Angular front and Spring Boot back in the Flame view](docs/images/front-back-flame.png)

### Dense view

For projects with many domains, the Dense view packs the matrix into uniform
tiles.

![Dense view of the same two projects](docs/images/dense.png)

## Quick start

### Install the browser extension from a release

Each [GitHub release](https://github.com/badetitou/Structure/releases/latest)
carries `structure-extension-<version>.zip`, a ready-built extension for
Chrome, Edge and Firefox. Unzip it and load the folder unpacked (see
[Load it unpacked](#build-the-browser-extension) below), then open a pull
request on GitHub and click the **Structure** tab. No Rust or Node needed.

The desktop app and the CLI are built from source for now.

### Prerequisites

- Rust (stable) and Node 20+
- For the desktop app: `cargo install tauri-cli --version "^2"`
- For Java indexing: a JDK, plus Maven or the project's Gradle wrapper. `scip-java` or Coursier (`cs`) is optional.
- For TypeScript indexing: `npx` (the indexer is fetched and run for you)
- For Pharo: nothing; Tonel sources (`src/<Package>/<Class>.class.st`) are read directly

### Run the desktop app

```sh
cd app/ui && npm install
cd ../src-tauri && cargo tauri dev
```

Click **Open project…**, choose a Java, TypeScript or Pharo folder, then **Analyze**.
Add a second folder to analyse a front and a back together.

To try the UI without analysing anything, run it in a browser on sample data:

```sh
cd app/ui && npm run dev                 # jpetstore sample
# then open /?sample=matrix-multi for the front + back sample
```

### Run the CLI

```sh
cargo run -p structure-cli -- analyze <project> -o matrix.json
```

Example on jpetstore-6:

```console
$ structure analyze fixtures/jpetstore-6 -o matrix.json
[  5%] indexing
[ 35%] parsing
[ 60%] linking
[ 80%] classifying
[ 95%] building matrix
[100%] done
language: Java, files: 25, symbols: 395, edges: 1092
scip: 45 documents, 9849 occurrences
edges: 594 (call 170, uses 424)
total: 85 ms
                  account         cart      catalog        order         root staxfactoryi
entry                   1            1            1            1            1            .
business                1            .            1            1            .            .
persistence             1            .            3            3            .            .
model                   1            2            3            3            .            .
util                    .            .            .            .            .            1
```

Progress, stats and the table go to stderr. The matrix JSON goes to `-o` or
stdout. (Output shortened; timings are with an existing `index.scip`.)

More commands:

```sh
structure analyze front/ back/ -o matrix.json     # several roots, one matrix
structure analyze <project> --no-scip             # tree-sitter only, no indexer
structure analyze <project> --reindex             # rebuild index.scip
structure impact <project> -o impact.json         # working tree vs HEAD
structure impact <project> --base main --head feature
```

`impact` returns the changed cells and the cells that depend on them, with
their distance from the change:

```json
{
  "base": "HEAD",
  "head": null,
  "changed_files": ["src/main/java/org/mybatis/jpetstore/mapper/AccountMapper.java"],
  "changed_cells": ["persistence:account"],
  "impacted_cells": [
    { "id": "business:account", "distance": 1 },
    { "id": "entry:account", "distance": 2 },
    { "id": "entry:order", "distance": 3 }
  ]
}
```

Full samples are in [docs/samples/](docs/samples/).

### Build the browser extension

The extension adds a **Structure** tab to GitHub pull requests. Opening it shows
the matrix at the PR head with its changed and impacted cells. Nothing runs on
your machine besides the browser: the core is compiled to WebAssembly, and the
PR diff and head sources are fetched from the GitHub API.

Extra prerequisites:

- `rustup target add wasm32-unknown-unknown`
- `cargo install wasm-pack`
- LLVM (`brew install llvm`): Apple's `clang`/`ar` cannot build tree-sitter for
  wasm. Elsewhere, set `LLVM_PREFIX`, or `CC_wasm32_unknown_unknown` and
  `AR_wasm32_unknown_unknown`.

Build the wasm module, then the extension:

```sh
scripts/build-wasm.sh                    # → crates/wasm/pkg/
cd app/ui && npm install && npm run build:extension   # → app/ui/dist-extension/
```

Rerun `scripts/build-wasm.sh` after any change to the Rust crates;
`build:extension` stops with an error if `crates/wasm/pkg` is missing.
`dist-extension/` holds one Manifest V3 build for both browsers.

Load it unpacked:

- **Chrome / Edge**: `chrome://extensions` → enable *Developer mode* →
  *Load unpacked* → pick `app/ui/dist-extension`.
- **Firefox** (140+): `about:debugging#/runtime/this-firefox` → *Load Temporary
  Add-on…* → pick `app/ui/dist-extension/manifest.json`. Temporary add-ons are
  removed when Firefox restarts. If Firefox has not granted access to
  `api.github.com` and `codeload.github.com`, the viewer shows a button that
  asks for it.

After a rebuild, click *Reload* on the extension and refresh the GitHub tab.

Open a pull request of a Java, TypeScript or Pharo (Tonel) repository and click the
**Structure** tab. Without a token, GitHub limits anonymous API calls to 60 per
hour and only public repositories work. To raise the limit or read private
repositories, add a GitHub token in the extension's options page; it is stored
locally and only sent to `api.github.com`.

## How it works

```mermaid
flowchart LR
    src[Source folder] --> ts[tree-sitter<br>symbols, annotations,<br>supertypes]
    src --> idx[SCIP indexer<br>scip-java / scip-typescript]
    idx --> scip[index.scip<br>exact references]
    ts --> link[Link<br>Call / Uses / Implements edges]
    scip --> link
    link --> classify[Classify<br>TOML rules → layer, domain]
    classify --> matrix[Quotient<br>layer × domain matrix]
    git[git diff] --> impact[Impact<br>changed + reverse reachability]
    matrix --> impact
    matrix --> ui[CLI JSON / Tauri app]
    impact --> ui
```

1. **Parse.** Tree-sitter (Java, TypeScript) or the Tonel reader (Pharo) extracts classes, ranges, annotations and supertypes.
2. **Index.** The SCIP indexer runs as a subprocess. An existing `<project>/index.scip` is reused. Pharo has no SCIP step: its call edges come from the Tonel reader.
3. **Link.** SCIP occurrences are joined to tree-sitter symbols to produce `Call`, `Uses` and `Implements` edges.
4. **Classify.** TOML rules assign each class a layer and a domain.
5. **Quotient.** Symbols collapse into cells; edges are aggregated between cells.
6. **Impact.** A git diff maps to changed symbols, then reverse edges give the impacted cells.

## Classification rules

Rules are TOML. The first matching rule wins; matchers inside a rule are ANDed.

```toml
[[rule]]
layer = "entry"
any_annotation = ["RestController", "Controller"]

[[rule]]
layer = "persistence"
kind = ["interface"]
name_suffix = ["Mapper"]

[[rule]]
layer = "util"
package_contains = [".config.", ".configuration."]
```

Built-in rules cover Spring, Stripes, MyBatis and JPA ([rules/java.toml](rules/java.toml))
Angular and NestJS ([rules/typescript.toml](rules/typescript.toml)), and
Pharo package naming conventions such as `-Model`, `-UI`, `-Tests`
([rules/pharo.toml](rules/pharo.toml)). To
override them for one project, add `<project>/.structure/rules.toml`: its rules
take priority over the built-in ones.

## Repository layout

```text
crates/
  ir/          Graph, Symbol, Edge, Matrix, Impact: the shared contracts
  lang-java/   tree-sitter front end for Java
  lang-ts/     tree-sitter front end for TypeScript
  lang-pharo/  Tonel reader for Pharo (classes, methods, sends)
  scip/        reads index.scip, links occurrences to symbols
  classify/    rules and the quotient matrix
  git/         diff → changed and impacted cells
  wasm/        WebAssembly build of the analysis, for the browser extension
  core/        facade and pipeline (analyze, indexers, snapshots, history)
  cli/         the `structure` binary
app/
  src-tauri/   Tauri 2 commands over the pipeline
  ui/          Vite + Svelte 5 + TypeScript; extension/ and src/extension/ hold the browser extension
rules/         built-in classification rules
docs/          plan, Tauri API contract, sample JSON
scripts/       fetch and index the demo fixtures, build the wasm module
tests/golden/  expected classifications
```

More detail: [docs/plan.md](docs/plan.md) (POC plan, in French),
[docs/api.md](docs/api.md) (UI ↔ Rust contract), [app/README.md](app/README.md) (desktop app).

## Development

```sh
cargo check --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app/ui && npm test && npm run check
```

End-to-end tests use demo projects in `fixtures/` (git-ignored):

```sh
scripts/fetch-fixtures.sh     # clone jpetstore-6, angular-realworld, spring-boot-realworld
scripts/index-fixtures.sh     # write fixtures/*/index.scip
STRUCTURE_FIXTURES=$PWD/fixtures cargo test
```

Tests skip a fixture that is missing or has no `index.scip`; they never run an
indexer. See [scripts/README.md](scripts/README.md).

## Limitations

- **Class-level granularity.** Classification is heuristic and driven by the rules.
- **Without `index.scip`** (or if indexing fails), only tree-sitter edges are used, so call edges are far fewer.
- **Impact follows reverse edges only.** No dynamic dispatch, reflection or DI wiring beyond what SCIP sees.
- **Language detection** uses `pom.xml` / `build.gradle*` (Java) or `tsconfig.json` / `package.json` (TypeScript), or a Tonel `src/` folder (Pharo). One language per root.
- **First indexing can take minutes** (Maven build, `npm install`).
- **scip-java runs the project's Maven build.** A `~/.m2/settings.xml` that points to an unreachable private repository makes it hang; the app gives up after 10 minutes and falls back to tree-sitter.
- **Gradle projects** are indexed through their own wrapper, which must support the JDK in use.
- **Kotlin sources are ignored**; generated sources under `build/` are not modelled.
- **Pharo call edges are heuristic.** Sends to other receivers resolve only when
  the selector is defined once in the project.
- **The browser extension uses tree-sitter only** (no SCIP), so Java and
  TypeScript have no call edges there: impact follows `Uses` and supertype edges. One root per PR, no HTTP or
  RPC links. It is not yet listed on the Chrome Web Store or addons.mozilla.org; install it
  from a [release](https://github.com/badetitou/Structure/releases/latest).
