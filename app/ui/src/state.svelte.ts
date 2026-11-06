import * as api from "./api";
import type { History, Impact, Matrix, Progress } from "./api";
import type { ViewCell } from "./domaintree";
import { PAGE, appendPage } from "./cellsymbols";
import type { SymbolList } from "./cellsymbols";
import { selectionDiff } from "./history";
import type { Selection } from "./history";
import type { Mode } from "./layout";
import { RequestGuard, shownMatrix } from "./matrixsel";
import { loadMode, saveMode } from "./mode";
import { loadPanels, savePanels } from "./panels";
import { analyzeAndRecord } from "./recent";

const START: Progress = { step: "starting…", pct: 0 };

/** State of the analysis screen: project, analysis, selection, impact, history, layout. */
export class AppState {
  project = $state("");
  /** "loading" while analysing, "ready" once the working-tree matrix is there. */
  phase = $state<"idle" | "loading" | "ready">("idle");
  progress = $state<Progress>(START);
  /** Working-tree matrix. */
  work = $state.raw<Matrix | null>(null);
  impact = $state.raw<Impact | null>(null);
  /** Selected (view) cell and its symbols, fetched a page at a time. */
  selected = $state.raw<ViewCell | null>(null);
  symbols = $state.raw<SymbolList | null>(null);
  mode = $state<Mode>(loadMode());
  /** Null when unavailable (not a git repo, error): the sidebar is hidden. */
  history = $state.raw<History | null>(null);
  historySel = $state.raw<Selection | null>(null);
  impactError = $state("");
  impactBusy = $state(false);
  panels = $state(loadPanels());

  private guard = new RequestGuard();
  private symReq = 0;
  private loadReq = 0;

  /** The matrix to display: the commit snapshot's when the impact carries one, else the working tree. */
  get matrix(): Matrix | null {
    return this.work ? shownMatrix(this.work, this.impact) : null;
  }
  get warnings(): string[] {
    return this.impact?.warnings ?? this.matrix?.warnings ?? [];
  }
  /** Commit whose snapshot is shown, null for the working tree. */
  private get shownHead(): string | null {
    return this.matrix === this.work ? null : (this.impact?.head ?? null);
  }

  /** Analyses `path` (progress in `progress`), then loads its history. Rejects when the analysis fails. */
  async load(path: string): Promise<void> {
    this.reset();
    const req = this.loadReq;
    this.project = path;
    this.phase = "loading";
    let m: Matrix;
    try {
      m = await analyzeAndRecord(path, (p) =>
        api.analyze(p, (pr) => {
          if (req === this.loadReq) this.progress = pr;
        }),
      );
    } catch (e) {
      if (req !== this.loadReq) return; // superseded: not this load's failure to report
      throw e;
    }
    if (req !== this.loadReq) return;
    this.work = m;
    this.phase = "ready";
    try {
      const h = await api.history(path);
      if (req === this.loadReq) this.history = h;
    } catch {
      // history is optional
    }
  }

  /** Back to the empty state; late answers of a previous load are ignored. */
  reset() {
    this.loadReq++;
    this.guard.invalidate();
    this.symReq++;
    this.project = "";
    this.phase = "idle";
    this.progress = START;
    this.work = null;
    this.impact = null;
    this.selected = null;
    this.symbols = null;
    this.history = null;
    this.historySel = null;
    this.impactError = "";
    this.impactBusy = false;
  }

  toggleMode() {
    this.mode = this.mode === "dense" ? "normal" : "dense";
    saveMode(this.mode);
  }

  savePanels() {
    savePanels(this.panels);
  }

  /** Select a view cell (or none) and list its symbols. */
  selectCell(cell: ViewCell | null) {
    this.selected = cell;
    void this.loadSymbols(false);
  }

  /** First page of the selected cell's symbols, or the next one when `more`. */
  async loadSymbols(more: boolean): Promise<void> {
    const cell = this.selected;
    const m = this.matrix;
    const req = ++this.symReq;
    if (!cell || !m) {
      this.symbols = null;
      return;
    }
    const prev = this.symbols;
    const base: SymbolList =
      more && prev?.cell === cell.id
        ? { ...prev, loading: true }
        : { cell: cell.id, total: cell.size, items: [], loading: true };
    this.symbols = base;
    try {
      const page = await api.cellSymbols(this.project, this.shownHead, m.generation ?? 0, cell.members, base.items.length, PAGE);
      if (req !== this.symReq) return;
      this.symbols = appendPage(base, page);
    } catch (e) {
      if (req !== this.symReq) return;
      this.symbols = { ...base, loading: false, error: String(e) };
    }
  }

  /** Diff of two refs (null = HEAD / working tree); shows the commit snapshot when there is one. */
  async runImpact(base: string | null, head: string | null): Promise<void> {
    const req = this.guard.begin();
    this.impactError = "";
    this.impactBusy = true;
    try {
      const res = await api.impact(this.project, base, head);
      if (!this.guard.isCurrent(req)) return; // superseded or cleared meanwhile
      this.setImpact(res);
    } catch (e) {
      if (this.guard.isCurrent(req)) this.impactError = `Impact failed: ${String(e)}`;
    } finally {
      if (this.guard.isCurrent(req)) this.impactBusy = false;
    }
  }

  clearImpact() {
    this.guard.invalidate();
    this.historySel = null;
    this.impactError = "";
    this.impactBusy = false;
    this.setImpact(null);
  }

  /** Select history entry `sel`; its diff drives the impact. */
  pickHistory(sel: Selection) {
    if (!this.history) return;
    const d = selectionDiff(this.history.commits, sel);
    if (!d) return;
    this.historySel = sel;
    void this.runImpact(d.base, d.head);
  }

  /** A typed diff is not a sidebar selection. */
  runTypedImpact(base: string | null, head: string | null) {
    this.historySel = null;
    void this.runImpact(base, head);
  }

  private setImpact(imp: Impact | null) {
    const before = this.matrix;
    this.impact = imp;
    if (this.matrix !== before) {
      // Another snapshot: cell ids are not comparable.
      this.selected = null;
      void this.loadSymbols(false);
    }
  }
}

export const app = new AppState();
