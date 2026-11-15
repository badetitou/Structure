// WebExtension namespace. Firefox exposes `browser` (promise based) and also `chrome`;
// Chrome only exposes `chrome`, whose MV3 methods used here return promises too.
// Source: MDN, "Build a cross-browser extension" / "Chrome incompatibilities".

export type ExtensionApi = typeof chrome;

/** `browser` when present, else `chrome`, else undefined (plain web page, tests). */
export function pickApi(scope: { browser?: unknown; chrome?: unknown }): ExtensionApi | undefined {
  return (scope.browser ?? scope.chrome) as ExtensionApi | undefined;
}

/** The extension API of the running browser; throws outside an extension context. */
export function ext(): ExtensionApi {
  const api = pickApi(globalThis as { browser?: unknown; chrome?: unknown });
  if (!api) throw new Error("WebExtension API not available in this context.");
  return api;
}
