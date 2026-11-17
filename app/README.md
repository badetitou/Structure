# Structure desktop app (Tauri 2)

The commands in `src-tauri/src/commands.rs` are thin wrappers over a managed
`structure_core::session::Session`, which owns the cached analyses (per
canonical path, and per commit for snapshots); they run it on a blocking
thread, emit its progress as `analysis-progress` events and return its errors
as strings. Pipeline warnings travel in the `warnings` field of `Matrix` and
`Impact`. While a project is open the UI polls `rules_stamp` every 2 s and
calls `reclassify` when the project's `.structure/rules.toml` changes (also
the "Reload rules" button of the Options page): `reloadRules` (`src/stores/session.ts`) keeps
the selection whose cell still exists, re-runs the impact shown, and on error
keeps the matrix and shows `rulesError` above it. In a plain browser the UI falls back to `docs/samples/*.json` (no rules reload there).

Prerequisites: Rust toolchain, `cargo install tauri-cli --version "^2"`, Node 20+,
platform webview deps (macOS: none extra).

## Run

```
cd app/ui && npm install
cd ../src-tauri && cargo tauri dev        # full desktop app
cd app/ui && npm run dev                  # browser only, uses /samples/*.json
cd app/src-tauri && cargo tauri build --debug --no-bundle   # binary in target/debug/structure-app
```

`app/ui/public/samples/` is a git-ignored copy of `docs/samples/`, refreshed by
`predev`/`prebuild`; it is the browser fallback (no `__TAURI_INTERNALS__`).
`npm test` runs vitest; `npm run check` runs svelte-check (`.ts` and `.svelte`; `npm run build` runs it too). Hovering a cell temporarily overrides impact colouring. The "HTTP links" toolbar toggle (kept in localStorage as `structure.httpEdges`, shown only when the matrix has front-to-back http links) shows or hides the dashed link edges; The "RPC links" toggle (`structure.rpcEdges`, shown only when the matrix has GWT RPC edges) does the same for dotted, purple RPC links (`/?sample=matrix-rpc`); in a plain browser `/?sample=matrix-multi` loads `docs/samples/matrix-multi.json`. The "Align projects" toggle (`structure.matrix.aligned`, default on, shown for matrices with several projects and remote links) puts a front domain and the back domain it calls on one row and gives each project its own layer segment, split by a double line (`aligned.ts`). The "Dense" toolbar toggle (kept in localStorage) packs the matrix into uniform tiles without labels or edges (`layout.ts`).

## Layout

- `src-tauri/` crate `structure-app` (standalone workspace, own Cargo.lock/target).
- `ui/` Vite + Svelte 5 + Tailwind v4 + shadcn-svelte. `App.svelte`/`Home.svelte`/`ThemeSwitch.svelte` are the shell; the analysis screen is `lib/components/AnalysisScreen.svelte` (one compact top bar `Toolbar` with the `ImpactRange` popover, Progress or the `workspace/Workspace.svelte` docking workspace, then `StatusBar` whose labels come from `src/statusbar.ts`) over focused Svelte 5 rune stores in `src/stores/`, each a class with one exported instance (backend calls via `api.ts`), which views reach only through the view context (below): `analysis.svelte.ts` (`analysis`: folders, phase, progress, working-tree matrix, rules reload and watch, `rulesError`, `multi` (several distinct roots; one root holding several languages is not multi), `rulesReloadable`), `impact.svelte.ts` (`impact`, an `ImpactOverlays`: the list of highlight overlays, the selected ones (`selectedIds` in click order; `selected` is the first, `selectedOverlays` all of them in list order, `multi` when two or more, `current` the impact drawn alone, null when several are), busy, progress, error, `run`/`add`/`select`/`pin`/`remove`/`clear`; session data, nothing persisted; the list rules are pure functions in `src/overlays.ts`: an impact replaces the unpinned overlay unless the toolbar's "Keep" pinned it, the same range updates its overlay in place, each overlay gets a colour token `--overlay-N` of `app.css`, removing the only selected one selects the most recent left, "Clear impact" drops the selection and the unpinned overlay; `OverlaySelector.svelte` shows the overlays as chips in a thin bar above the matrix (in `MatrixPanel`) only when two or more are loaded (or when a kept one is left unselected by "Clear impact", so it can still be selected or removed); a click shows that overlay alone, cmd-click (macOS) / ctrl-click adds or removes it from the ones shown (never emptying the selection; rules in `overlays.ts`); one shown: the matrix (its commit snapshot), status bar, details, history selection and diff follow it as before; several shown: they are drawn on the working-tree matrix (snapshot cells are mapped by id, those it lacks are counted in the status bar), each cell hit by up to three is split into bands in the overlays' colours (solid with a top mark when changed, faded by distance when impacted), more than three take the neutral `--overlay-multiple` style, the cell tooltip lists them, the details panel shows per-overlay hits of the selected cell or a per-overlay summary, and the diff follows the first selected; the pure helpers (per-cell aggregation, band geometry, texts) are in `src/multioverlay.ts`; after a rules reload the overlays not shown are marked stale and computed again when selected, the shown ones are computed again one after the other; the viewer shows its single impact with no selector), `selection.svelte.ts` (`selection`: selected cell, its symbols paged, the drawn view matrix, `sync`), `git.svelte.ts` (`git`: history, history selection, diff, `historyAvailable`/`diffAvailable`), `options.svelte.ts` (`options`: the global display defaults — mode, transposed, aligned, HTTP/RPC links — and the app-wide angled labels, persisted in localStorage) `shown.ts` (`shown`: the matrix shown, the snapshot's or the working tree's, its warnings and head; read-only), `lenses.svelte.ts` (`lenses`: the lenses the backend offers, asked once per project load, and, fetched on first read and cached, the matrix shown through a non-default lens — by lens, commit and generation — and each impact's cells in it, which the backend computes from the impact's symbols; the default lens needs nothing; entries no view can show any more — older generations, snapshots and overlays no longer listed — are pruned by `session.ts`, and a failure is kept until retried; pure rules in `src/lens.ts`) and `progress.ts` (`START`, the progress shown before a run's first report, shared by analysis and impact without coupling them). Actions that touch several stores (load / reset, run / clear an impact, pick a history entry, reload the rules then re-run the impact and keep the selection) are the plain functions of `src/stores/session.ts`; there is no facade object. The workspace is [dockview](https://dockview.dev) (tabs, drag-and-drop docking, resizable groups): `workspace/Workspace.svelte` creates it, `src/workspace/dock.ts` mounts registered Svelte panels (`SvelteRenderer`), hides groups whose panels are all unavailable and the tabs of unavailable panels in mixed groups (the panels stay in dockview, so the saved layout keeps them), and persists the layout under `structure.dock`; its colours are mapped to the theme tokens in `app.css` (`.dockview-theme-structure`). Panels are contributions registered in `src/workspace/registry.ts` (`panels.register({ id, title, component, available?, multi?, display? })`). A `multi` contribution can be open several times: the first instance has the contribution's id, the others `<id>-N` (`nextInstanceId` in `src/workspace/paneldisplay.ts`); the Views menu lists the extra open instances (unchecking closes one) and offers "New <title> view", which opens one more beside the active one (`workspace.openNew`); availability and pruning of a restored layout are asked of a panel's contribution (`byComponent` in `dock.ts`). A `display` contribution (the matrix) owns its display options per instance: mode, transposed, aligned, HTTP/RPC links and the lens live in the panel's dockview params (`params.display`, saved with the layout under `structure.dock`); a panel without them (the first matrix panel of a layout saved before options were per panel, a new view, the default layout after a reset) adopts the global defaults of `options` once and writes them to its params. Every change also updates the global defaults, so with a single matrix panel the choices persist as before. Two matrix panels can thus be open side by side with different options. The lens (`params.display.lens`, default `layer-domain`) is how the backend quotients the graph (`layer-domain`: inferred domains; `layer-package`: raw packages / directories as a tree); it is not a global default (a new panel starts on `layer-domain`), a lens the backend does not offer falls back to the default (a panel saved with another lens shows "Loading …" until the backend has listed its lenses, rather than the default lens first), and the toolbar shows a lens selector for the active panel only when the backend offers more than one (`capabilities.lenses`, refreshed by `Backend.lenses`). A panel's context gives its matrix (`lens.matrix`, `lens.work`) and its overlays' cells in its lens (loading: no cells drawn, and the panel keeps its last matrix of that lens drawn, with its open groups and zoom, under a "Loading …" badge; failure: `lens.error` for the matrix, `overlays.lensError` for overlays it could not map, both shown in the panel with a Retry button: overlays whose commit snapshot the backend no longer holds are computed again, other failures are requested again). Expanding or collapsing a group drops the shared selection only in the panel it was made in. The selection stays shared and remembers its lens (`selection.lens`): its symbols are listed in that lens, and a panel of another lens neither draws nor drops it but says "Selected cell not in this view". The built-ins (`matrix` → `MatrixPanel`, `details` → `DetailsPanel`, `history` → `HistorySidebar`) and the default layout are in `src/workspace/builtin.ts`; a new panel is one more `register` call, reads and acts through the view context, and shows up in the toolbar's Views menu. Views (panels and the components they render, plus the toolbar, its popovers and the status bar) never import the stores: they call `getViewContext()` (`src/workspace/context.ts`), a `ViewContext` with a reactive read side (`lens`: paths, working-tree and shown matrix, head, warnings, display options; `overlays`; `selection`: selected cell, its symbols, the drawn view matrix; `git`; `rules`; `capabilities`) and `actions` (select a cell, load more symbols, publish the drawn view, select / keep / remove / clear overlays, run a typed range, pick a history entry, reload the rules, set the display options) that delegate to the stores and `session.ts` functions, with no state or behaviour of its own. Selection and overlays are the shared bus every view reads. Display options are per panel: `Workspace.svelte` mounts a `display` panel with `ctx.forPanel(id, params, persist)` (a context whose `lens.display` and display setters are that panel's, `panel` its id) using `SvelteRenderer`'s per-instance context factory, and tells `ctx.displays` (`src/workspace/displays.svelte.ts`, pure rules in `paneldisplay.ts`) which panel dockview activates and which closes. The base context (toolbar, status bar, details, …) reads and sets the active matrix panel's options — the focused one, else the last focused still open, else the global defaults; zoom / fit act on it too (`matrixControl` by panel id). Each matrix panel publishes its own drawn view: `selection.view` is the active panel's, and only the active panel keeps the shared selection in step with its cells. `AnalysisScreen` creates it (`createViewContext`) and provides it with Svelte's `setContext` (`App.svelte` provides one too, for the home bar's Options button); dockview panels are separate Svelte roots, so `Workspace.svelte` hands it to `SvelteRenderer`, which passes it as `mount`'s `context` (no props). Only `AnalysisScreen` (owns the session: load, reset, rules watch) and `Progress` (shown before any view) import the stores; `src/workspace/views.test.ts` scans the sources (the components and the `.ts` modules they import, stopping at `context.ts`) to keep it so; besides the stores, only `context.ts` and `builtin.ts` import them. `builtin.ts` reads store getters for panel availability (workspace glue, not a view). The matrix is a component tree: `MatrixView.svelte` (derives columns/layout/bands from the matrix, mode and expanded groups; owns zoom/pan as one `transform`) over `matrix/ColumnHeaders`, `matrix/MatrixCell`, `matrix/MatrixEdge`. Hover/impact/selection are per-cell/edge derivations (`cellstate.ts`, adjacency built once in `graph.ts`); the containers carry `hovering`/`impacting` classes that dim the unaffected cells and edges, so a hover only writes to the ones that change. Long symbol lists in the details panel show 200 at a time ("Show more"). `src/app.css` is the single theme file (shadcn variables for light and `.dark`, plus matrix/impact/lane tokens); no colour literals elsewhere, SVG uses `var(--…)` via `svgattr.ts`. Light/dark/system is handled by `mode-watcher`. Add a component with `cd ui && npx shadcn-svelte@latest add <name>` (lands in `src/lib/components/ui`, alias `$lib`). `typescript` is pinned to ^6 because `svelte-check` does not support TS 7 yet. `src/api.ts` typed wrappers for the API in `docs/api.md`, behind a swappable `Backend` (Tauri, browser samples, or `src/viewer/backend.ts` for the embedded viewer and the extension) that declares its `capabilities` (`history`, `diff`, `pickFolder`, `reclassify`, `highlightSources`, `lenses`) and leaves out the methods it does not support; panel availability and the rules controls derive from them (`git.historyAvailable`, `git.diffAvailable`, `analysis.rulesReloadable`); its wire types come from `src/generated/` (generated from `crates/ir` by ts-rs, do not edit; see `docs/api.md`).

## Publishing the extension

`.github/workflows/extension-release.yml` runs on a `v*` tag. It sets the manifest
`version` from the tag (`v0.2.0` → `0.2.0`; 1 to 4 dot-separated integers, higher
than the previous release on each store), builds the wasm package and the
extension, and attaches two files to the GitHub release:
`structure-extension-<version>.zip` (the extension) and
`structure-extension-<version>-source.zip` (the tagged sources, for AMO review;
how to rebuild them is in `ui/extension/BUILD.md`). It then submits the
extension to Firefox Add-ons (`web-ext sign --channel listed`, the review is not
awaited) and uploads and publishes it on the Chrome Web Store
(`chrome-webstore-upload-cli`). Both tools run through `npx` at a pinned version,
in CI only. A store whose secrets are missing is skipped with a notice.

```
git tag v0.2.0 && git push origin v0.2.0
```

### One-time setup

The stores' APIs only update existing items, so the first version of each is
submitted by hand. Push a first tag before adding any secret: the release gets
the two zips, which you then upload yourself.

**Firefox Add-ons (AMO)**

1. Sign in at <https://addons.mozilla.org/developers/> (Firefox account).
2. *Submit a New Add-on* → *On this site* (listed) → upload
   `structure-extension-<version>.zip`. When asked whether the code is
   minified or generated, answer yes and upload
   `structure-extension-<version>-source.zip`. Fill in the listing (summary,
   category, license, support links). The add-on ID is the manifest's
   `structure@structure.local`; it cannot change afterwards.
3. *Tools → Manage API Keys* (<https://addons.mozilla.org/developers/addon/api/key/>)
   → generate credentials. The *JWT issuer* goes in `AMO_JWT_ISSUER`, the
   *JWT secret* in `AMO_JWT_SECRET`.

**Chrome Web Store**

1. Register a developer account at
   <https://chrome.google.com/webstore/devconsole> (one-time registration fee).
2. *New item* → upload `structure-extension-<version>.zip`. Fill in the store
   listing and the *Privacy* tab: single purpose, a justification for
   `storage` (the options page keeps the GitHub token) and for the host
   permissions (`api.github.com` and `codeload.github.com`: PR diff and head
   sources), data usage (the token is stored locally and only sent to
   `api.github.com`), and a **privacy policy URL** — a public page saying the
   same, e.g. a Markdown file in this repository. Submit for review.
3. Copy the item's ID (`CWS_EXTENSION_ID`) and, from the console's *Account*
   page, the publisher ID (`CWS_PUBLISHER_ID`).
4. API access: in a Google Cloud project, enable the *Chrome Web Store API*,
   configure the OAuth consent screen (external) and **publish it to
   production** (refresh tokens of a consent screen in *Testing* expire after
   7 days), then create an OAuth client of type *Desktop app*: its ID and
   secret are `CWS_CLIENT_ID` and `CWS_CLIENT_SECRET`. Get `CWS_REFRESH_TOKEN`
   with the guide at <https://github.com/fregante/chrome-webstore-upload-keys>,
   signed in with the account that owns the item.

**Secrets**

Add them as repository secrets (*Settings → Secrets and variables → Actions*),
or with the CLI, which prompts for each value:

```
gh secret set AMO_JWT_ISSUER
gh secret set AMO_JWT_SECRET
gh secret set CWS_EXTENSION_ID
gh secret set CWS_PUBLISHER_ID
gh secret set CWS_CLIENT_ID
gh secret set CWS_CLIENT_SECRET
gh secret set CWS_REFRESH_TOKEN
```

Firefox needs both `AMO_*` secrets, Chrome all five `CWS_*` secrets.
