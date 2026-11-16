// Host access for the viewer page. Firefox treats MV3 `host_permissions` as user controlled
// (shown at install since Firefox 127, but the user may refuse or revoke them), so the page
// checks them and asks through `permissions.request`, which must run in a user action handler.
// Source: MDN, manifest.json/host_permissions and API/permissions/request.

/** Origins the viewer fetches from (pull request API, then its redirect to the tarball host). */
export const REQUIRED_ORIGINS = ["https://api.github.com/*", "https://codeload.github.com/*"];

/** The slice of `permissions` used here. */
export interface PermissionsApi {
  contains(p: { origins: string[] }): Promise<boolean>;
  request(p: { origins: string[] }): Promise<boolean>;
}

/** True when the origins are available; an API failure counts as available (the fetch will report). */
export async function hasHostAccess(p: PermissionsApi, origins: string[] = REQUIRED_ORIGINS): Promise<boolean> {
  try {
    return await p.contains({ origins });
  } catch {
    return true;
  }
}

/** Shows the browser's own prompt. Call it first thing in a click handler. */
export async function requestHostAccess(p: PermissionsApi, origins: string[] = REQUIRED_ORIGINS): Promise<boolean> {
  try {
    return await p.request({ origins });
  } catch {
    return false;
  }
}
