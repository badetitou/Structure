---
id: 85
title: Extension works in Firefox
status: in-review
parent: 76
branch: 85-firefox-extension
remote: null
created: 2026-11-15
---
## Context

The extension of #82 is built and checked for Chrome only (Manifest V3, `chrome.*`
calls, `app/ui/dist-extension`). Firefox also supports Manifest V3 but differs on a few
points: an add-on id (`browser_specific_settings.gecko`), how host permissions are
granted, the `browser.*` namespace, and how a page's CSP applies to extension frames.

(Ids 83 and 84 are taken by the "Uses edges" issues written in another worktree.)

## Acceptance criteria
- [ ] The extension loads in Firefox as a temporary add-on (`about:debugging` →
  "Load Temporary Add-on") without manifest errors, and still loads in Chrome.
- [ ] On a GitHub pull request the "Structure" tab appears and the viewer shows the
  matrix, with the same pipeline as in Chrome. When Firefox has not granted the host
  permissions (`github.com`, `api.github.com`, `codeload.github.com`), the viewer says
  so and offers a button that requests them.
- [ ] The options page stores the token in Firefox.
- [ ] `npm run check`, `npm test`, `npm run build`, `npm run build:extension` green;
  the Firefox manifest passes `web-ext lint` without errors.
- [ ] Manual check in Firefox on a public Java PR.

## Out of scope
- Publication on addons.mozilla.org, signing.
- Firefox for Android, Safari.
