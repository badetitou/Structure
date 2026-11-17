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
// Provided once per analysis screen (`AnalysisScreen.svelte` calls `setViewContext`); the
// dockview panels, mounted as separate Svelte roots, receive it through `mount`'s `context`
// option (see `SvelteRenderer` in dock.ts), so a panel and its children all call
// `getViewContext()` and need no props.
import { getContext, setContext } from "svelte";
import * as api from "../api";
import type { Capabilities, Impact, Matrix, Progress } from "../api";
import type { SymbolList } from "../cellsymbols";
import type { ViewCell, ViewMatrix } from "../domaintree";
import type { Selection } from "../history";
import type { Mode } from "../layout";
import type { Overlay, Range } from "../overlays";
import { analysis } from "../stores/analysis.svelte";
import { git } from "../stores/git.svelte";
import type { DiffState } from "../stores/git.svelte";
import { impact } from "../stores/impact.svelte";
import { options } from "../stores/options.svelte";
import { selection } from "../stores/selection.svelte";
import * as session from "../stores/session";
import { shown } from "../stores/shown";

/** How the matrix is drawn (global today, saved in localStorage). */
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
  /** Folders analysed together. */
  readonly paths: readonly string[];
  /** First folder: the project for single-project code. */
  readonly project: string;
  /** Several distinct roots analysed together (no git views). */
  readonly multi: boolean;
  /** Working-tree matrix (the base a commit snapshot is compared to), null until analysed. */
  readonly work: Matrix | null;
  /** Matrix shown: the impact's commit snapshot, else the working tree. */
  readonly matrix: Matrix | null;
  /** Commit whose snapshot is shown, null for the working tree. */
  readonly head: string | null;
  readonly warnings: readonly string[];
  /** Several overlays drawn: the cells they hit that the matrix shown lacks. */
  readonly missingCells: number;
  readonly display: DisplayView;
}

/** The highlight overlays (shared by every view). */
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
}

/** The selected cell (shared by every view) and its symbols. */
export interface SelectionView {
  readonly selected: ViewCell | null;
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
}

/** The view context backed by the app's stores and session functions. */
export function createViewContext(): ViewContext {
  const display: DisplayView = {
    get mode() {
      return options.mode;
    },
    get transposed() {
      return options.transposed;
    },
    get aligned() {
      return options.aligned;
    },
    get showHttp() {
      return options.showHttp;
    },
    get showRpc() {
      return options.showRpc;
    },
    get angledLabels() {
      return options.angledLabels;
    },
  };
  const lens: LensView = {
    get paths() {
      return analysis.paths;
    },
    get project() {
      return analysis.project;
    },
    get multi() {
      return analysis.multi;
    },
    get work() {
      return analysis.work;
    },
    get matrix() {
      return shown.matrix;
    },
    get head() {
      return shown.head;
    },
    get warnings() {
      return shown.warnings;
    },
    get missingCells() {
      return shown.missingCells;
    },
    display,
  };
  const overlays: OverlaysView = {
    get list() {
      return impact.overlays;
    },
    get selectedIds() {
      return impact.selectedIds;
    },
    get selected() {
      return impact.selected;
    },
    get selectedOverlays() {
      return impact.selectedOverlays;
    },
    get multi() {
      return impact.multi;
    },
    get current() {
      return impact.current;
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
  };
  const sel: SelectionView = {
    get selected() {
      return selection.selected;
    },
    get symbols() {
      return selection.symbols;
    },
    get view() {
      return selection.view;
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
    selectCell: (cell) => selection.select(cell),
    loadMoreSymbols: () => selection.loadSymbols(true),
    publishView: (view) => {
      selection.view = view;
    },
    syncSelection: (cells) => selection.sync(cells),
    selectOverlay: (id, toggle = false) => session.selectOverlay(id, toggle),
    keepOverlays: () => session.keepOverlay(),
    removeOverlay: (id) => session.removeOverlay(id),
    clearImpact: () => session.clearImpact(),
    runImpact: (base, head) => session.runTypedImpact(base, head),
    pickHistory: (s) => session.pickHistory(s),
    reloadRules: () => session.reloadRules(),
    setMode: (mode) => options.setMode(mode),
    setTransposed: (on) => options.setTransposed(on),
    setAligned: (on) => options.setAligned(on),
    setShowHttp: (on) => options.setShowHttp(on),
    setShowRpc: (on) => options.setShowRpc(on),
    setAngledLabels: (on) => options.setAngledLabels(on),
  };
  return {
    lens,
    overlays,
    selection: sel,
    git: gitView,
    rules,
    get capabilities() {
      return api.capabilities();
    },
    actions,
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
