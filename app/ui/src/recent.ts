/** One analysis: the set of folders analysed together (one for a single project). */
export type RecentProject = { paths: string[]; name: string; lastOpened: string };

export const MAX_RECENT = 8;
const KEY = "structure.recentProjects";

export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

const asPaths = (p: string | string[]): string[] => (typeof p === "string" ? [p] : p);

/** Identity of a set of folders, independent of their order. */
export function recentKey(paths: string | string[]): string {
  return [...asPaths(paths)].sort().join("\n");
}

export function addRecent(
  list: RecentProject[],
  paths: string | string[],
  now: Date = new Date(),
): RecentProject[] {
  const ps = asPaths(paths);
  const entry = { paths: ps, name: ps.map(folderName).join(" + "), lastOpened: now.toISOString() };
  const key = recentKey(ps);
  return [entry, ...list.filter((r) => recentKey(r.paths) !== key)].slice(0, MAX_RECENT);
}

export function removeRecent(list: RecentProject[], paths: string | string[]): RecentProject[] {
  const key = recentKey(paths);
  return list.filter((r) => recentKey(r.paths) !== key);
}

/** Accepts the current shape and the old `{path}` one (migrated to `paths: [path]`). */
function migrate(r: unknown): RecentProject | null {
  if (!r || typeof r !== "object") return null;
  const o = r as Record<string, unknown>;
  if (typeof o.name !== "string" || typeof o.lastOpened !== "string") return null;
  const paths = Array.isArray(o.paths) ? o.paths : typeof o.path === "string" ? [o.path] : null;
  if (!paths || paths.length === 0 || !paths.every((p) => typeof p === "string")) return null;
  return { paths: paths as string[], name: o.name, lastOpened: o.lastOpened };
}

export function parseRecent(raw: string | null): RecentProject[] {
  if (!raw) return [];
  try {
    const v: unknown = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return v
      .map(migrate)
      .filter((r): r is RecentProject => r !== null)
      .slice(0, MAX_RECENT);
  } catch {
    return [];
  }
}

export function loadRecent(): RecentProject[] {
  try {
    return parseRecent(localStorage.getItem(KEY));
  } catch {
    return [];
  }
}

export function saveRecent(list: RecentProject[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // storage unavailable: recents are best-effort
  }
}

export type RecentStore = {
  load(): RecentProject[];
  save(list: RecentProject[]): void;
};

const defaultStore: RecentStore = { load: loadRecent, save: saveRecent };

/** Runs `analyze`, then records `paths`; on failure rethrows and leaves the store untouched. */
export async function analyzeAndRecord<T>(
  paths: string[],
  analyze: (paths: string[]) => Promise<T>,
  store: RecentStore = defaultStore,
  now: Date = new Date(),
): Promise<T> {
  const result = await analyze(paths);
  store.save(addRecent(store.load(), paths, now));
  return result;
}
