// The view context: what a view (a workspace panel, or the bars around the workspace) reads
// and what it can ask for, instead of reaching into the global stores. Today it is a thin
// facade over the stores and the session functions (no state of its own, no behaviour of its
// own): every read is a getter on a store, so it stays reactive in a component, and every
// action delegates to the store method or session function the components called before.
//
// Selection and overlays are the shared bus: every view reads the same `selection` and
// `overlays`, so a cell selected in the matrix is the one the details panel lists, and an
// overlay picked in the selector is the one the history and diff follow.
//
// Display options are per panel instance: each matrix panel gets its own context
// (`forPanel`), whose `lens.display` and display setters are that panel's (stored in its
// dockview params, see displays.svelte.ts). The base context (toolbar, status bar, the other
// panels) reads and sets the active matrix panel's (the focused one, else the last focused),
// falling back to the global defaults while no matrix panel is open.
//
// The lens (the backend's quotient, `lens.ts`) is one of those per-panel options: a context's
// `lens.matrix` is the matrix shown through its lens, and its `overlays` carry the impacts'
// cells in that lens (the backend maps the impacts' symbols, see `stores/lenses.svelte.ts`),
// so a view draws them as it always did. The selection remembers the lens it was made in: a
// panel showing another lens does not draw it (and says so) nor keeps it in step.
//
// Provided once per analysis screen (`AnalysisScreen.svelte` calls `setViewContext`); the
// dockview panels, mounted as separate Svelte roots, receive it through `mount`'s `context`
// option (see `SvelteRenderer` in dock.ts), so a panel and its children all call
// `getViewContext()` and need no props.
import { getContext, setContext } from "svelte";
import * as api from "../api";
import type { Capabilities, Impact, LensInfo, Matrix, Progress } from "../api";
import type { SymbolList } from "../cellsymbols";
import type { ViewCell, ViewMatrix } from "../domaintree";
import type { Selection } from "../history";
import type { Mode } from "../layout";
import { DEFAULT_LENS, lensLabel, resolveLens } from "../lens";
import { cellsNotIn } from "../multioverlay";
import type { Overlay, Range } from "../overlays";
import { analysis } from "../stores/analysis.svelte";
import { git } from "../stores/git.svelte";
import type { DiffState } from "../stores/git.svelte";
import { impact } from "../stores/impact.svelte";
import { lenses } from "../stores/lenses.svelte";
import { options } from "../stores/options.svelte";
import { selection } from "../stores/selection.svelte";
import * as session from "../stores/session";
import { shown } from "../stores/shown";
import { PanelDisplays } from "./displays.svelte";
import type { PanelDisplay } from "./paneldisplay";

/** How the matrix is drawn: the panel's own options (see `forPanel`), angled labels app-wide. */
export interface DisplayView {
  readonly mode: Mode;
  readonly transposed: boolean;
  readonly aligned: boolean;
  readonly showHttp: boolean;
  readonly showRpc: boolean;
  readonly angledLabels: boolean;
}

/** The lens: the analysis and the matrix shown, with what a view needs to derive its view model. */
export interface LensView {
  /** Id of the lens drawn (the panel's, or the active panel's): one the backend offers. */
  readonly id: string;
  readonly label: string;
  /** The lenses the backend offers, the default first. */
  readonly available: readonly LensInfo[];
  /** The lens matrix could not be loaded ("" when fine or still loading). */
  readonly error: string;
  /** Folders analysed together. */
  readonly paths: readonly string[];
  /** First folder: the project for single-project code. */
  readonly project: string;
  /** Several distinct roots analysed together (no git views). */
  readonly multi: boolean;
  /** Working-tree matrix as analysed (default lens), null until analysed: what does not
   * depend on the lens (project names, language), there while a lens matrix loads. */
  readonly analysed: Matrix | null;
  /** Working-tree matrix (the base a commit snapshot is compared to) through the lens, null
   * until analysed (or while the lens matrix loads). */
  readonly work: Matrix | null;
  /** Matrix shown, through the lens: the impact's commit snapshot, else the working tree. */
  readonly matrix: Matrix | null;
  /** Commit whose snapshot is shown, null for the working tree. */
  readonly head: string | null;
  readonly warnings: readonly string[];
  /** Several overlays drawn: the cells they hit that the matrix shown lacks. */
  readonly missingCells: number;
  readonly display: DisplayView;
}

/** The highlight overlays (shared by every view), their impacts' cells in the context's lens. */
export interface OverlaysView {
  /** Loaded overlays, oldest first. */
  readonly list: readonly Overlay[];
  /** Ids of the overlays shown, in selection order. */
  readonly selectedIds: readonly string[];
  /** The first selected overlay, null when none. */
  readonly selected: Overlay | null;
  /** The selected overlays, in list order. */
  readonly selectedOverlays: readonly Overlay[];
  /** Several overlays drawn together. */
  readonly multi: boolean;
  /** The impact drawn alone, null when none or several are shown. */
  readonly current: Impact | null;
  /** Range of the impact running, else of the first one shown. */
  readonly range: Range | null;
  /** An impact runs: overlay and history actions are ignored meanwhile. */
  readonly busy: boolean;
  readonly progress: Progress | null;
  readonly error: string;
  /** Overlays shown whose cells the context's lens could not get ("" when none): they are drawn
   * empty in it (see `ViewActions.retryLens`). */
  readonly lensError: string;
}

/** The selected cell (shared by every view) and its symbols. */
export interface SelectionView {
  readonly selected: ViewCell | null;
  /** Lens the selected cell belongs to (its ids are that lens' cells). */
  readonly lens: string;
  readonly symbols: SymbolList | null;
  /** The aggregated matrix drawn by the matrix view (names the visible cells). */
  readonly view: ViewMatrix | null;
}

/** Git side of a single project. */
export interface GitView {
  readonly history: api.History | null;
  readonly historySel: Selection | null;
  readonly diff: DiffState | null;
  readonly historyAvailable: boolean;
  readonly diffAvailable: boolean;
}

/** The project's classification rules reload. */
export interface RulesView {
  readonly reloadable: boolean;
  readonly busy: boolean;
  readonly error: string;
}

/** What a view can ask for. */
export interface ViewActions {
  /** Selects a view cell (or none) and lists its symbols. */
  selectCell(cell: ViewCell | null): void;
  /** Next page of the selected cell's symbols. */
  loadMoreSymbols(): Promise<void>;
  /** The matrix view publishes the aggregated matrix it draws (null when it goes away). */
  publishView(view: ViewMatrix | null): void;
  /** Keeps the selection in step with the cells drawn. */
  syncSelection(cells: ViewCell[]): void;
  /** The view's cell ids changed (a group expanded or collapsed): the selection is dropped
   * when it is this view's (made in its lens and, for a panel, while it is the active one). */
  expansionChanged(): void;
  /** Shows overlay `id` alone, or with `toggle` adds / removes it from the ones shown. */
  selectOverlay(id: string, toggle?: boolean): void;
  /** Keeps the overlays shown when the next impact runs. */
  keepOverlays(): void;
  removeOverlay(id: string): void;
  /** Drops the impact shown, the history selection and the diff. */
  clearImpact(): void;
  /** Impact (and diff) of a typed range; null = HEAD / working tree. */
  runImpact(base: string | null, head: string | null): void;
  /** Selects a history entry; its diff drives the impact. */
  pickHistory(sel: Selection): void;
  reloadRules(): Promise<void>;
  setMode(mode: Mode): void;
  setTransposed(on: boolean): void;
  setAligned(on: boolean): void;
  setShowHttp(on: boolean): void;
  setShowRpc(on: boolean): void;
  /** Shows the panel (the active one, for the base context) through lens `id`. */
  setLens(id: string): void;
  /** Retries what the context's lens could not load (see `session.retryLens`). */
  retryLens(): void;
  setAngledLabels(on: boolean): void;
}

/** Everything a view gets: the read side (reactive getters) and its actions. */
export interface ViewContext {
  readonly lens: LensView;
  readonly overlays: OverlaysView;
  readonly selection: SelectionView;
  readonly git: GitView;
  readonly rules: RulesView;
  /** What the backend supports. */
  readonly capabilities: Capabilities;
  readonly actions: ViewActions;
  /** The dockview panel this context belongs to (`forPanel`), null for the base context. */
  readonly panel: string | null;
  /** Display options of the panel instances (workspace glue: focus / close, the active panel). */
  readonly displays: PanelDisplays;
  /**
   * The context of panel `id`, which owns its display options: read from its dockview
   * `params` (the global defaults when it has none), written back with `persist`. Selection
   * and overlays stay the shared ones.
   */
  forPanel(id: string, params: unknown, persist: (d: PanelDisplay) => void): ViewContext;
}

/** The view context backed by the app's stores and session functions. */
export function createViewContext(): ViewContext {
  const displays = new PanelDisplays(
    () => ({
      mode: options.mode,
      transposed: options.transposed,
      aligned: options.aligned,
      showHttp: options.showHttp,
      showRpc: options.showRpc,
      // A new panel starts on the default lens: the lens is not a global preference.
      lens: DEFAULT_LENS,
    }),
    (patch) => {
      if (patch.mode !== undefined) options.setMode(patch.mode);
      if (patch.transposed !== undefined) options.setTransposed(patch.transposed);
      if (patch.aligned !== undefined) options.setAligned(patch.aligned);
      if (patch.showHttp !== undefined) options.setShowHttp(patch.showHttp);
      if (patch.showRpc !== undefined) options.setShowRpc(patch.showRpc);
    },
  );
  // The base context's display: the active matrix panel's, else the global defaults.
  const own = (): Omit<PanelDisplay, "lens"> => displays.active ?? options;
  const display = displayOf(own);
  const baseLens = () => resolveLens(displays.active?.lens ?? DEFAULT_LENS, lenses.available, lenses.known);
  const lens = lensOf(display, baseLens);
  const overlays = overlaysOf(baseLens);
  const sel: SelectionView = {
    get selected() {
      return selection.selected;
    },
    get lens() {
      return selection.lens;
    },
    get symbols() {
      return selection.symbols;
    },
    get view() {
      // A matrix panel publishes its view per panel: the active one names the cells.
      return displays.activeId !== null ? displays.activeView : selection.view;
    },
  };
  const gitView: GitView = {
    get history() {
      return git.history;
    },
    get historySel() {
      return git.historySel;
    },
    get diff() {
      return git.diff;
    },
    get historyAvailable() {
      return git.historyAvailable;
    },
    get diffAvailable() {
      return git.diffAvailable;
    },
  };
  const rules: RulesView = {
    get reloadable() {
      return analysis.rulesReloadable;
    },
    get busy() {
      return analysis.rulesBusy;
    },
    get error() {
      return analysis.rulesError;
    },
  };
  const actions: ViewActions = {
    selectCell: (cell) => selection.select(cell, baseLens()),
    loadMoreSymbols: () => selection.loadSymbols(true),
    publishView: (view) => {
      selection.view = view;
    },
    syncSelection: (cells) => selection.sync(cells),
    expansionChanged: () => {
      if (selection.lens === baseLens()) selection.select(null, baseLens());
    },
    selectOverlay: (id, toggle = false) => session.selectOverlay(id, toggle),
    keepOverlays: () => session.keepOverlay(),
    removeOverlay: (id) => session.removeOverlay(id),
    clearImpact: () => session.clearImpact(),
    runImpact: (base, head) => session.runTypedImpact(base, head),
    pickHistory: (s) => session.pickHistory(s),
    reloadRules: () => session.reloadRules(),
    setMode: (mode) => displays.set({ mode }),
    setTransposed: (on) => displays.set({ transposed: on }),
    setAligned: (on) => displays.set({ aligned: on }),
    setShowHttp: (on) => displays.set({ showHttp: on }),
    setShowRpc: (on) => displays.set({ showRpc: on }),
    setLens: (id) => displays.set({ lens: id }),
    retryLens: () => session.retryLens(baseLens()),
    setAngledLabels: (on) => options.setAngledLabels(on),
  };
  const make = (panel: string | null, lens: LensView, overlays: OverlaysView, actions: ViewActions): ViewContext => ({
    lens,
    overlays,
    selection: sel,
    git: gitView,
    rules,
    get capabilities() {
      return api.capabilities();
    },
    actions,
    panel,
    displays,
    forPanel,
  });
  function forPanel(id: string, params: unknown, persist: (d: PanelDisplay) => void): ViewContext {
    const state = displays.attach(id, params, persist);
    const set = (patch: Partial<PanelDisplay>) => displays.set(patch, id);
    const panelLens = () => resolveLens(state.lens, lenses.available, lenses.known);
    // The shared selection is this panel's: made in its lens, and the panel is the active one.
    const ownsSelection = () => displays.activeId === id && selection.lens === panelLens();
    const panelActions: ViewActions = {
      ...actions,
      selectCell: (cell) => selection.select(cell, panelLens()),
      // Each matrix panel draws its own view; the active one's is the shared `selection.view`,
      // and only the active one keeps the shared selection in step with its cells.
      publishView: (view) => displays.publishView(id, view),
      // A selection made in another lens is not this panel's to drop.
      syncSelection: (cells) => {
        if (ownsSelection()) selection.sync(cells);
      },
      // Another panel's selection does not depend on this panel's expansion.
      expansionChanged: () => {
        if (ownsSelection()) selection.select(null, panelLens());
      },
      setMode: (mode) => set({ mode }),
      setTransposed: (on) => set({ transposed: on }),
      setAligned: (on) => set({ aligned: on }),
      setShowHttp: (on) => set({ showHttp: on }),
      setShowRpc: (on) => set({ showRpc: on }),
      setLens: (lens) => set({ lens }),
      retryLens: () => session.retryLens(panelLens()),
    };
    return make(id, lensOf(displayOf(() => state), panelLens), overlaysOf(panelLens), panelActions);
  }
  return make(null, lens, overlays, actions);
}

/** The overlays with their impacts' cells in the lens `lensId` names (the impact store's own
 * objects for the default lens). */
function overlaysOf(lensId: () => string): OverlaysView {
  const lensed = (o: Overlay): Overlay => {
    const l = lensId();
    if (l === DEFAULT_LENS) return o;
    const i = lenses.impact(l, o.impact);
    return i === o.impact ? o : { ...o, impact: i };
  };
  const own = () => lensId() === DEFAULT_LENS;
  return {
    get list() {
      return own() ? impact.overlays : impact.overlays.map(lensed);
    },
    get selectedIds() {
      return impact.selectedIds;
    },
    get selected() {
      const o = impact.selected;
      return o ? lensed(o) : null;
    },
    get selectedOverlays() {
      return own() ? impact.selectedOverlays : impact.selectedOverlays.map(lensed);
    },
    get multi() {
      return impact.multi;
    },
    get current() {
      const c = impact.current;
      return c ? lenses.impact(lensId(), c) : null;
    },
    get range() {
      return impact.range;
    },
    get busy() {
      return impact.busy;
    },
    get progress() {
      return impact.progress;
    },
    get error() {
      return impact.error;
    },
    get lensError() {
      const l = lensId();
      if (l === DEFAULT_LENS) return "";
      const failed = impact.selectedOverlays.filter((o) => lenses.impactError(l, o.impact) !== "");
      if (failed.length === 0) return "";
      const names = failed.map((o) => o.label).join(", ");
      return `Cannot draw ${names} in ${lensLabel(l, lenses.available)}: ${lenses.impactError(l, failed[0].impact)}`;
    },
  };
}

/** Display read through `own` (a panel's options, or the active / global ones), angled labels app-wide. */
function displayOf(own: () => Omit<PanelDisplay, "lens">): DisplayView {
  return {
    get mode() {
      return own().mode;
    },
    get transposed() {
      return own().transposed;
    },
    get aligned() {
      return own().aligned;
    },
    get showHttp() {
      return own().showHttp;
    },
    get showRpc() {
      return own().showRpc;
    },
    get angledLabels() {
      return options.angledLabels;
    },
  };
}

/** The lens `lensId` names read from the stores, with `display` as its display options. */
function lensOf(display: DisplayView, lensId: () => string): LensView {
  return {
    get id() {
      return lensId();
    },
    get label() {
      return lensLabel(lensId(), lenses.available);
    },
    get available() {
      return lenses.available;
    },
    get error() {
      return lenses.matrixError(lensId(), shown.matrix, shown.head);
    },
    get paths() {
      return analysis.paths;
    },
    get project() {
      return analysis.project;
    },
    get multi() {
      return analysis.multi;
    },
    get analysed() {
      return analysis.work;
    },
    get work() {
      return lenses.matrix(lensId(), analysis.work, null);
    },
    get matrix() {
      return lenses.matrix(lensId(), shown.matrix, shown.head);
    },
    get head() {
      return shown.head;
    },
    get warnings() {
      return shown.warnings;
    },
    get missingCells() {
      const l = lensId();
      if (l === DEFAULT_LENS) return shown.missingCells;
      const m = lenses.matrix(l, shown.matrix, shown.head);
      return impact.multi && m ? cellsNotIn(impact.selectedOverlays.map((o) => lenses.impact(l, o.impact)), m) : 0;
    },
    display,
  };
}

const KEY = Symbol("structure.view-context");

/** Provides `ctx` to the calling component and its children (call during component init). */
export function setViewContext(ctx: ViewContext): ViewContext {
  return setContext(KEY, ctx);
}

/** The view context provided by an ancestor (call during component init). */
export function getViewContext(): ViewContext {
  const ctx = getContext<ViewContext | undefined>(KEY);
  if (!ctx) throw new Error("no view context: mount the view under AnalysisScreen or pass it to mount()");
  return ctx;
}

/** `ctx` as the `context` map of Svelte's `mount`, for views mounted as their own root (dockview panels). */
export function viewContextMap(ctx: ViewContext): Map<symbol, ViewContext> {
  return new Map([[KEY, ctx]]);
}
