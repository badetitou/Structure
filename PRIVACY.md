# Privacy policy

*Last updated: 16 November 2026*

This policy covers the **Structure** browser extension (Chrome, Edge and Firefox),
which adds a *Structure* tab to GitHub pull requests.

## Summary

The extension does not collect, transmit, sell or share any personal data with the
author or with any third party. Everything it computes stays in your browser.

## What the extension does

When you open the *Structure* tab on a GitHub pull request, the extension:

1. asks the GitHub API (`api.github.com`) for the pull request's metadata and diff;
2. downloads the repository sources at the pull request's head from GitHub
   (`codeload.github.com`);
3. analyses them locally in your browser (WebAssembly) to display the pull
   request's matrix and impact.

No server operated by the author is involved. The extension has no analytics,
telemetry, advertising or tracking of any kind.

## Data stored on your device

If you choose to add a GitHub personal access token on the extension's options
page, it is stored in the browser's extension storage (`chrome.storage.local`) on
your device only. It is not synced and not sent to the author.

The token is used to raise GitHub's rate limit and to read private repositories
you have access to. It is sent **only** to `api.github.com`, in the request
headers. You can remove it at any time by clearing the field on the options page
or by uninstalling the extension.

The extension also keeps UI preferences (such as the light or dark theme) in the
browser's local storage. They contain no personal data.

## Permissions

- **`storage`**: keeps the optional GitHub token and your preferences on your
  device.
- **Host access to `api.github.com` and `codeload.github.com`**: read the pull
  request's diff and the repository sources, as described above.
- **Content script on `github.com`**: adds the *Structure* tab to pull request
  pages. The extension does not read or modify any other page.

## Data sent to GitHub

Requests to GitHub are subject to [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
GitHub sees them as ordinary API calls from your browser, authenticated with your
token if you provided one.

## Changes

If this policy changes, the new version will be published in this file, in the
project repository.

## Contact

Questions or concerns: open an issue at
<https://github.com/badetitou/Structure/issues>.
