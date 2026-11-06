export type RecentProject = { path: string; name: string; lastOpened: string };

export const MAX_RECENT = 8;
const KEY = "structure.recentProjects";

export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function addRecent(
  list: RecentProject[],
  path: string,
  now: Date = new Date(),
): RecentProject[] {
  const entry = { path, name: folderName(path), lastOpened: now.toISOString() };
  return [entry, ...list.filter((r) => r.path !== path)].slice(0, MAX_RECENT);
}

export function removeRecent(list: RecentProject[], path: string): RecentProject[] {
  return list.filter((r) => r.path !== path);
}

export function parseRecent(raw: string | null): RecentProject[] {
  if (!raw) return [];
  try {
    const v: unknown = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return v
      .filter(
        (r): r is RecentProject =>
          !!r &&
          typeof r.path === "string" &&
          typeof r.name === "string" &&
          typeof r.lastOpened === "string",
      )
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

/** Runs `analyze`, then records `path`; on failure rethrows and leaves the store untouched. */
export async function analyzeAndRecord<T>(
  path: string,
  analyze: (path: string) => Promise<T>,
  store: RecentStore = defaultStore,
  now: Date = new Date(),
): Promise<T> {
  const result = await analyze(path);
  store.save(addRecent(store.load(), path, now));
  return result;
}
