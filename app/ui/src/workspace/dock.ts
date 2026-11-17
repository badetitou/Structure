// Glue between the panel registry and dockview: how a registered Svelte panel is
// mounted, which groups are shown, and how the layout is persisted.
import type {
  DockviewApi,
  DockviewOptions,
  DockviewTheme,
  GroupPanelPartInitParameters,
  IContentRenderer,
  SerializedDockview,
} from "dockview";
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

/** The Svelte context of a panel: fixed, or built from its init parameters (its params and api). */
export type PanelContext = Map<unknown, unknown> | ((init: GroupPanelPartInitParameters) => Map<unknown, unknown>);

/**
 * Mounts a Svelte component (no props) as a dockview panel's content. A panel is its own
 * Svelte root, so the Svelte context it reads (the view context, see context.ts) is handed
 * over explicitly as `context`; a function builds it per panel instance when mounted.
 */
export class SvelteRenderer implements IContentRenderer {
  readonly element: HTMLElement;
  private instance: ReturnType<typeof mount> | null = null;

  constructor(
    private readonly component: Component,
    private readonly context?: PanelContext,
  ) {
    this.element = document.createElement("div");
    this.element.className = "dock-panel";
  }

  init(init: GroupPanelPartInitParameters): void {
    if (this.instance) return;
    const context = typeof this.context === "function" ? this.context(init) : this.context;
    this.instance = mount(this.component, { target: this.element, context });
  }

  dispose(): void {
    if (this.instance) void unmount(this.instance);
    this.instance = null;
  }
}

/** What `syncVisibility` and `pruneUnknown` need of dockview (kept small to test without a DOM). */
export interface DockLike {
  groups: {
    panels: { id: string; api?: { setActive(): void } }[];
    activePanel?: { id: string };
    api: { isVisible: boolean; setVisible(v: boolean): void };
  }[];
  panels: { id: string; api: { close(): void } }[];
}

/**
 * Keeps unavailable panels out of sight (e.g. history outside a git repository, or for
 * a multi-project analysis). A group whose panels are all unavailable is hidden so it
 * takes no room. In a group that mixes both, the unavailable panels' tabs are hidden
 * through `setTabHidden` and an available panel is activated if one was showing.
 * The panels stay in dockview, so the saved layout keeps them and they reappear in
 * place once available.
 */
export function syncVisibility(
  dock: DockLike,
  available: (panel: string) => boolean,
  setTabHidden: (panel: string, hidden: boolean) => void = () => {},
): void {
  for (const g of dock.groups) {
    const show = g.panels.length === 0 || g.panels.some((p) => available(p.id));
    if (g.api.isVisible !== show) g.api.setVisible(show);
    for (const p of g.panels) setTabHidden(p.id, !available(p.id));
    if (g.activePanel && !available(g.activePanel.id)) g.panels.find((p) => available(p.id))?.api?.setActive();
  }
}

/**
 * `pred` asked of a panel's contribution (its dockview component) rather than its id: the
 * instances of a multi-instance panel (`matrix`, `matrix-2`, …) share one. `component` gives
 * a panel's component, undefined when unknown (the id is used then).
 */
export function byComponent(
  component: (panel: string) => string | undefined,
  pred: (contribution: string) => boolean,
): (panel: string) => boolean {
  return (panel) => pred(component(panel) ?? panel);
}

/** Hides / shows the tab of `panel` inside the dockview mounted in `host`. */
export function setTabHiddenIn(host: HTMLElement, panel: string, hidden: boolean): void {
  const tab = host.querySelector<HTMLElement>(`.dv-tab[data-tab-panel-id="${CSS.escape(panel)}"]`);
  if (!tab) return;
  const display = hidden ? "none" : "";
  if (tab.style.display !== display) tab.style.display = display;
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
