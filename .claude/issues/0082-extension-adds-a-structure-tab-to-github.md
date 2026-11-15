---
id: 82
title: Extension adds a Structure tab to GitHub pull requests
status: in-review
parent: 76
branch: 82-github-pr-extension
remote: null
created: 2026-11-13
---
## Context

Last slice of #76: the extension that ties the wasm module (#80) and the viewer (#81)
to github.com.

Design notes (2026-11-15):
- Everything runs in the viewer page (an extension page): it fetches from GitHub with the
  extension's host permissions, runs the wasm module and keeps the analysis in memory for
  cell symbols. No service worker.
- `codeload.github.com` only allows `render.githubusercontent.com` as CORS origin, so
  the tarball can only be fetched from an extension page (not from a dev server page).
- GitHub's CSP is `frame-src viewscreen.githubusercontent.com notebooks.githubusercontent.com`;
  Chrome exempts an extension's web-accessible pages, but an "open in a new tab" link is
  kept next to the embedded view in case the frame is blocked.
- PR tab bar on 2026-11-15: `nav[aria-label="Pull request navigation"]` > `div` >
  `a#prs-files-anchor-tab` (hashed class names: copy the classes of that link at runtime).
- The extension is a second build of `app/ui` (`npm run build:extension` →
  `app/ui/dist-extension`), so it shares the viewer code and its dependencies.

## Acceptance criteria
- [ ] `npm run build:extension` (in `app/ui`) produces `dist-extension/`, loadable
  unpacked in Chrome (Manifest V3). Host permissions: `api.github.com` and
  `codeload.github.com` only; content script on `github.com`; `storage` permission.
- [ ] On a pull request page a "Structure" tab is added after "Files changed" and
  survives GitHub's soft navigation between PR tabs and between pull requests.
- [ ] Opening it shows the viewer under the tab bar: it reads the PR (base / head sha),
  its diff and the head sources (tarball, only `.java` / `.ts` / `.tsx` kept), runs the
  wasm analysis and shows the matrix with changed / impacted cells. Progress and errors
  (rate limit, private repo without token, unsupported language, repository too large,
  diff too large) are shown in the viewer.
- [ ] Going back to another PR tab and reopening "Structure" does not re-analyse while
  the page stays on the same pull request.
- [ ] Options page stores an optional GitHub token (`chrome.storage.local`), sent to
  `api.github.com` only.
- [ ] Unit tests for the pure parts (PR URL parsing, tar reading and filtering, GitHub
  error mapping); a Node script runs the same pipeline against a public pull request.
- [ ] Manual check in Chrome on a public Java PR and a public TypeScript PR.

## Out of scope
- Firefox / Safari, store publication.
- Persistent cache across page loads; a Web Worker for the analysis.
- Linking a cell to the "Files changed" tab (follow-up).
