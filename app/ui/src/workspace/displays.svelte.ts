// The display options of the panel instances that own one (matrix panels), reactive, with the
// focus order that names the panel the toolbar acts on. One per view context (no store
// imports: the global defaults it falls back to and saves are handed in by context.ts).
import { SvelteMap } from "svelte/reactivity";
import type { ViewMatrix } from "../domaintree";
import type { Mode } from "../layout";
import { activeOf, enlist, forget, parseDisplay, snapshot, touch } from "./paneldisplay";
import type { PanelDisplay } from "./paneldisplay";

/** One panel's display options; every change is written back through `persist` (its dockview params). */
export class PanelDisplayState implements PanelDisplay {
  mode = $state<Mode>("normal");
  transposed = $state(false);
  aligned = $state(true);
  showHttp = $state(true);
  showRpc = $state(true);

  constructor(
    d: PanelDisplay,
    private persist: (d: PanelDisplay) => void,
  ) {
    this.assign(d);
  }

  private assign(d: Partial<PanelDisplay>) {
    if (d.mode !== undefined) this.mode = d.mode;
    if (d.transposed !== undefined) this.transposed = d.transposed;
    if (d.aligned !== undefined) this.aligned = d.aligned;
    if (d.showHttp !== undefined) this.showHttp = d.showHttp;
    if (d.showRpc !== undefined) this.showRpc = d.showRpc;
  }

  /** The panel was mounted again (same id): writes go to its new params. */
  rebind(persist: (d: PanelDisplay) => void) {
    this.persist = persist;
  }

  set(patch: Partial<PanelDisplay>) {
    this.assign(patch);
    this.persist(snapshot(this));
  }
}

/**
 * Display options by panel id. A panel is attached when mounted (its params read, or the
 * global defaults adopted and written to its params once), focused when dockview activates it,
 * dropped when closed. The active panel is the focused one, else the last focused still open.
 * Every change also updates the global defaults, which a panel without stored options (a new
 * view, the default layout after a reset) starts from.
 */
export class PanelDisplays {
  private states = new SvelteMap<string, PanelDisplayState>();
  private views = new SvelteMap<string, ViewMatrix>();
  /** Attached panels, most recently focused first. */
  recent = $state.raw<string[]>([]);
  /** Focused before being attached (dockview can activate a panel before mounting it). */
  private pendingFocus: string | null = null;

  constructor(
    private readonly defaults: () => PanelDisplay,
    private readonly saveDefaults: (patch: Partial<PanelDisplay>) => void,
  ) {}

  /** The display of panel `id`, from its dockview `params`; `persist` writes it back to them. */
  attach(id: string, params: unknown, persist: (d: PanelDisplay) => void): PanelDisplayState {
    const existing = this.states.get(id);
    if (existing) {
      existing.rebind(persist);
      return existing;
    }
    const { display, stored } = parseDisplay(params, this.defaults());
    const s = new PanelDisplayState(display, persist);
    this.states.set(id, s);
    this.recent = this.pendingFocus === id ? touch(this.recent, id) : enlist(this.recent, id);
    if (this.pendingFocus === id) this.pendingFocus = null;
    // Migration / new panel: the global defaults it adopted become its own, saved with the layout.
    if (!stored) persist(snapshot(display));
    return s;
  }

  get(id: string): PanelDisplayState | undefined {
    return this.states.get(id);
  }

  /** Panel `id` got the focus (ignored for panels with no display). */
  focus(id: string) {
    if (!this.states.has(id)) {
      this.pendingFocus = id;
      return;
    }
    this.pendingFocus = null;
    if (this.recent[0] !== id) this.recent = touch(this.recent, id);
  }

  /** Panel `id` was closed. */
  drop(id: string) {
    this.states.delete(id);
    this.views.delete(id);
    if (this.recent.includes(id)) this.recent = forget(this.recent, id);
  }

  /** The workspace went away: no panel left. */
  clear() {
    this.states.clear();
    this.views.clear();
    this.recent = [];
    this.pendingFocus = null;
  }

  get activeId(): string | null {
    return activeOf(this.recent);
  }

  get active(): PanelDisplayState | null {
    const id = this.activeId;
    return id === null ? null : (this.states.get(id) ?? null);
  }

  /** Sets options of panel `id` (the active one when null) and the global defaults. */
  set(patch: Partial<PanelDisplay>, id: string | null = null) {
    const s = id === null ? this.active : this.states.get(id);
    s?.set(patch);
    this.saveDefaults(patch);
  }

  /** Panel `id` draws `view` (null: it stopped drawing). */
  publishView(id: string, view: ViewMatrix | null) {
    if (view) this.views.set(id, view);
    else this.views.delete(id);
  }

  /** The view matrix drawn by the active panel. */
  get activeView(): ViewMatrix | null {
    const id = this.activeId;
    return id === null ? null : (this.views.get(id) ?? null);
  }
}
