# Structure desktop app (Tauri 2)

Commands `analyze` / `impact` call `structure_core::pipeline` (see
`src-tauri/src/commands.rs`); analyses are cached per canonical path in managed
state. Pipeline warnings are emitted as `analysis-progress` steps prefixed
`warning: `. In a plain browser the UI falls back to `docs/samples/*.json`.

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
- `ui/` Vite + Svelte 5 + Tailwind v4 + shadcn-svelte. `App.svelte`/`Home.svelte`/`ThemeSwitch.svelte` are the shell; the analysis screen is `lib/components/AnalysisScreen.svelte` (one compact top bar `Toolbar` with the `ImpactRange` popover, Progress or the `workspace/Workspace.svelte` docking workspace, then `StatusBar` whose labels come from `src/statusbar.ts`) over one runes state module, `src/state.svelte.ts` (`app`: project, analysis, selection, impact, history, mode; backend calls via `api.ts`). The workspace is [dockview](https://dockview.dev) (tabs, drag-and-drop docking, resizable groups): `workspace/Workspace.svelte` creates it, `src/workspace/dock.ts` mounts registered Svelte panels (`SvelteRenderer`), hides groups whose panels are all unavailable and the tabs of unavailable panels in mixed groups (the panels stay in dockview, so the saved layout keeps them), and persists the layout under `structure.dock`; its colours are mapped to the theme tokens in `app.css` (`.dockview-theme-structure`). Panels are contributions registered in `src/workspace/registry.ts` (`panels.register({ id, title, component, available? })`). The built-ins (`matrix` → `MatrixPanel`, `details` → `DetailsPanel`, `history` → `HistorySidebar`) and the default layout are in `src/workspace/builtin.ts`; a new panel is one more `register` call, reads what it needs from `app`, and shows up in the toolbar's Views menu. The matrix is a component tree: `MatrixView.svelte` (derives columns/layout/bands from the matrix, mode and expanded groups; owns zoom/pan as one `transform`) over `matrix/ColumnHeaders`, `matrix/MatrixCell`, `matrix/MatrixEdge`. Hover/impact/selection are per-cell/edge derivations (`cellstate.ts`, adjacency built once in `graph.ts`); the containers carry `hovering`/`impacting` classes that dim the unaffected cells and edges, so a hover only writes to the ones that change. Long symbol lists in the details panel show 200 at a time ("Show more"). `src/app.css` is the single theme file (shadcn variables for light and `.dark`, plus matrix/impact/lane tokens); no colour literals elsewhere, SVG uses `var(--…)` via `svgattr.ts`. Light/dark/system is handled by `mode-watcher`. Add a component with `cd ui && npx shadcn-svelte@latest add <name>` (lands in `src/lib/components/ui`, alias `$lib`). `typescript` is pinned to ^6 because `svelte-check` does not support TS 7 yet. `src/api.ts` typed wrappers for the API in `docs/api.md`.

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
