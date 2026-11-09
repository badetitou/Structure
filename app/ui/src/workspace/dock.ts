// Glue between the panel registry and dockview: how a registered Svelte panel is
// mounted, which groups are shown, and how the layout is persisted.
import type { DockviewApi, DockviewOptions, DockviewTheme, IContentRenderer, SerializedDockview } from "dockview";
import { mount, unmount } from "svelte";
import type { Component } from "svelte";

export const DOCK_KEY = "structure.dock";

/** Dockview colours come from the app's theme tokens (app.css, `.dockview-theme-structure`). */
export const structureTheme: DockviewTheme = {
  name: "structure",
  className: "dockview-theme-structure",
  dndOverlayMounting: "relative",
};

/**
 * Drag-and-drop runs on pointer events, not HTML5 drag-and-drop: the Tauri webview
 * (WKWebView) handles native drags itself, so HTML5 drags of tabs never reach dockview
 * there. Dockview's own docs advise `pointer` for embedded webviews.
 */
export const dockDnd = {
  dndStrategy: "pointer",
} as const satisfies Pick<DockviewOptions, "dndStrategy">;

/** Mounts a Svelte component (no props) as a dockview panel's content. */
export class SvelteRenderer implements IContentRenderer {
  readonly element: HTMLElement;
  private instance: ReturnType<typeof mount> | null = null;

  constructor(private readonly component: Component) {
    this.element = document.createElement("div");
    this.element.className = "dock-panel";
  }

  init(): void {
    if (!this.instance) this.instance = mount(this.component, { target: this.element });
  }

  dispose(): void {
    if (this.instance) void unmount(this.instance);
    this.instance = null;
  }
}

/** What `syncVisibility` and `pruneUnknown` need of dockview (kept small to test without a DOM). */
export interface DockLike {
  groups: { panels: { id: string }[]; api: { isVisible: boolean; setVisible(v: boolean): void } }[];
  panels: { id: string; api: { close(): void } }[];
}

/**
 * Hides the groups whose panels are all unavailable (e.g. history outside a git
 * repository) so they take no room, and shows the others. Only touches groups that change.
 */
export function syncVisibility(dock: DockLike, available: (panel: string) => boolean): void {
  for (const g of dock.groups) {
    const show = g.panels.length === 0 || g.panels.some((p) => available(p.id));
    if (g.api.isVisible !== show) g.api.setVisible(show);
  }
}

/** Closes panels no plugin provides any more (a restored layout can name a removed one). */
export function pruneUnknown(dock: DockLike, known: (panel: string) => boolean): void {
  for (const p of [...dock.panels]) if (!known(p.id)) p.api.close();
}

/** The stored layout, or null when missing, unreadable or not a dockview layout. */
export function parseDock(raw: string | null): SerializedDockview | null {
  if (raw === null) return null;
  try {
    const v: unknown = JSON.parse(raw);
    if (typeof v !== "object" || v === null) return null;
    const o = v as Record<string, unknown>;
    return typeof o.grid === "object" && o.grid !== null && typeof o.panels === "object" && o.panels !== null
      ? (v as SerializedDockview)
      : null;
  } catch {
    return null;
  }
}

export function loadDock(): SerializedDockview | null {
  try {
    return parseDock(localStorage.getItem(DOCK_KEY));
  } catch {
    return null;
  }
}

export function saveDock(api: DockviewApi): void {
  try {
    localStorage.setItem(DOCK_KEY, JSON.stringify(api.toJSON()));
  } catch {
    // storage unavailable: the layout just is not persisted
  }
}

/**
 * Restores `stored` into `api`, else (nothing stored, or dockview rejects it) lays out
 * the default. Panels no plugin provides are closed afterwards.
 */
export function restore(
  api: DockviewApi,
  stored: SerializedDockview | null,
  known: (panel: string) => boolean,
  defaults: (api: DockviewApi) => void,
): void {
  if (stored) {
    try {
      api.fromJSON(stored);
      pruneUnknown(api, known);
      if (api.totalPanels > 0) return;
    } catch {
      // fall through to the default layout
    }
    api.clear();
  }
  defaults(api);
}
