import * as api from "./api";
import type { Diff, History, Impact, Matrix, Progress } from "./api";
import type { ViewCell, ViewMatrix } from "./domaintree";
import { PAGE, appendPage } from "./cellsymbols";
import type { SymbolList } from "./cellsymbols";
import { selectionDiff } from "./history";
import type { Selection } from "./history";
import type { Mode } from "./layout";
import { RequestGuard, retainSelection, shownMatrix } from "./matrixsel";
import {
  loadAligned,
  loadAngled,
  loadMode,
  loadShowHttp,
  loadShowRpc,
  loadTransposed,
  saveAligned,
  saveAngled,
  saveMode,
  saveShowHttp,
  saveShowRpc,
  saveTransposed,
} from "./mode";
import { analyzeAndRecord } from "./recent";

/** The diff panel's content: the requested range and its diff once loaded. */
export interface DiffState {
  base: string | null;
  head: string | null;
  loading: boolean;
  data?: Diff;
  error?: string;
}

const START: Progress = { step: "starting…", pct: 0 };

/** How often the project's rules files are checked for changes while a project is open. */
export const RULES_POLL_MS = 2000;

/** State of the analysis screen: project, analysis, selection, impact, history. */
export class AppState {
  /** Folders analysed together (one for a single project). */
  paths = $state<string[]>([]);
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
  /** The aggregated matrix currently drawn (set by MatrixView; tracks expansion). */
  view = $state.raw<ViewMatrix | null>(null);
  /** Draw front-to-back HTTP links (persisted). */
  showHttp = $state(loadShowHttp());
  /** Draw GWT RPC links (persisted). */
  showRpc = $state(loadShowRpc());
  /** Domains as rows and layers as columns. */
  transposed = $state(loadTransposed());
  /** Multi-project matrix: front and back domains share rows, projects get their own layer segment (persisted). */
  aligned = $state(loadAligned());
  /** Option: rotated top labels drawn at 45 degrees instead of upright (persisted). */
  angledLabels = $state(loadAngled());
  /** Null when unavailable (not a git repo, error): the sidebar is hidden. */
  history = $state.raw<History | null>(null);
  historySel = $state.raw<Selection | null>(null);
  impactError = $state("");
  /** True while an impact runs (it may analyse a commit snapshot first); new diffs are ignored meanwhile. */
  impactBusy = $state(false);
  /** Progress of the running impact, null when none runs. */
  impactProgress = $state<Progress | null>(null);

  /** Textual diff of the history selection (or typed range), null when none is selected. */
  diff = $state.raw<DiffState | null>(null);

  /** Last rules reload failure (invalid rules file…); the matrix shown is kept. */
  rulesError = $state("");
  /** True while the rules are being reloaded. */
  rulesBusy = $state(false);
  /** Last seen stamp of the rules files, null until known. */
  private rulesStamp: string | null = null;
  private rulesReq = 0;
  /** Range of the impact shown (or running), re-run after a rules reload. */
  private impactRange: { base: string | null; head: string | null } | null = null;

  private guard = new RequestGuard();
  private diffGuard = new RequestGuard();
  private symReq = 0;
  private loadReq = 0;

  /** First folder: the project for single-project code. */
  get project(): string {
    return this.paths[0] ?? "";
  }
  /** Several distinct projects analysed together (per the loaded matrix, as the backend dedups roots): git panels are unavailable. */
  get multi(): boolean {
    return (this.work?.projects?.length ?? 0) > 1;
  }

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

  /** Analyses `paths` (progress in `progress`), then loads its history (single project only). Rejects when the analysis fails. */
  async load(input: string | string[]): Promise<void> {
    const paths = typeof input === "string" ? [input] : input;
    this.reset();
    const req = this.loadReq;
    this.paths = paths;
    this.phase = "loading";
    // Taken before the analysis so that an edit made meanwhile is reloaded.
    const stamp = this.readRulesStamp(paths);
    let m: Matrix;
    try {
      m = await analyzeAndRecord(paths, (p) =>
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
    void stamp.then((s) => {
      if (req === this.loadReq && this.rulesStamp === null) this.rulesStamp = s;
    });
    if (this.multi) return; // no git history across several repos
    try {
      const h = await api.history(paths[0]);
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
    this.paths = [];
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
    this.impactProgress = null;
    this.impactRange = null;
    this.rulesReq++;
    this.rulesError = "";
    this.rulesBusy = false;
    this.rulesStamp = null;
    this.clearDiff();
  }

  /** Stamp of the rules files of `paths`, null when unknown (unsupported or failed). */
  private async readRulesStamp(paths: string[]): Promise<string | null> {
    try {
      return (await api.rulesStamp(paths)) ?? null;
    } catch {
      return null;
    }
  }

  /** Re-applies the project's rules to the analysis without re-parsing it. The selection is
   * kept when its cell still exists and the impact shown is computed again; on failure the
   * matrix stays and the error goes to `rulesError`. */
  async reloadRules(): Promise<void> {
    if (this.phase !== "ready" || !this.work) return;
    const load = this.loadReq;
    const req = ++this.rulesReq;
    const current = () => load === this.loadReq && req === this.rulesReq;
    this.rulesBusy = true;
    try {
      // Stamp of the rules applied now, so the next poll does not reload them again.
      const stamp = await this.readRulesStamp(this.paths);
      if (!current()) return;
      if (stamp !== null) this.rulesStamp = stamp;
      const m = await api.reclassify(this.paths);
      if (!current()) return;
      this.rulesError = "";
      this.applyReclassified(m);
    } catch (e) {
      if (current()) this.rulesError = `Rules reload failed: ${String(e)}`;
    } finally {
      if (current()) this.rulesBusy = false;
    }
  }

  private applyReclassified(m: Matrix) {
    const sel = this.selected;
    const showsWork = this.matrix === this.work;
    this.work = m;
    if (showsWork) {
      // The old generation is gone: list the kept cell's symbols again.
      this.selected = sel ? retainSelection(sel, m) : null;
      void this.loadSymbols(false);
    }
    const range = this.impactRange;
    if (range) {
      // Computed with the old classification (a running one too): run it again.
      this.guard.invalidate();
      this.impactBusy = false;
      void this.runImpact(range.base, range.head, true);
    }
  }

  /** Reloads the rules when their files changed since last checked (polled while a project is open). */
  async checkRules(): Promise<void> {
    if (this.phase !== "ready" || this.rulesBusy) return;
    const load = this.loadReq;
    const stamp = await this.readRulesStamp(this.paths);
    if (load !== this.loadReq || stamp === null) return;
    const prev = this.rulesStamp;
    this.rulesStamp = stamp;
    if (prev !== null && prev !== stamp) await this.reloadRules();
  }

  /** Polls the rules files every `RULES_POLL_MS`; returns the function that stops it. */
  watchRules(): () => void {
    const id = setInterval(() => void this.checkRules(), RULES_POLL_MS);
    return () => clearInterval(id);
  }

  /** Keeps the selection in step with the cells drawn: dropped when its cell left the view,
   * replaced by the view's cell of the same id when that one differs (after a rules reload). */
  syncSelection(cells: ViewCell[]) {
    const sel = this.selected;
    if (!sel) return;
    const next = cells.find((c) => c.id === sel.id);
    if (!next) this.selectCell(null);
    else if (next.size !== sel.size || next.members.join("\n") !== sel.members.join("\n")) this.selectCell(next);
  }

  setShowHttp(on: boolean) {
    this.showHttp = on;
    saveShowHttp(on);
  }
  setShowRpc(on: boolean) {
    this.showRpc = on;
    saveShowRpc(on);
  }
  setMode(mode: Mode) {
    this.mode = mode;
    saveMode(mode);
  }

  setAligned(on: boolean) {
    this.aligned = on;
    saveAligned(on);
  }

  setAngledLabels(on: boolean) {
    this.angledLabels = on;
    saveAngled(on);
  }

  setTransposed(on: boolean) {
    this.transposed = on;
    saveTransposed(on);
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
      const page = await api.cellSymbols(this.paths, this.shownHead, m.generation ?? 0, cell.members, base.items.length, PAGE);
      if (req !== this.symReq) return;
      this.symbols = appendPage(base, page);
    } catch (e) {
      if (req !== this.symReq) return;
      this.symbols = { ...base, loading: false, error: String(e) };
    }
  }

  /** Diff of two refs (null = HEAD / working tree); shows the commit snapshot when there is one.
   * Ignored while another impact runs. */
  async runImpact(base: string | null, head: string | null, keepSelection = false): Promise<void> {
    if (this.impactBusy || this.multi) return;
    const req = this.guard.begin();
    this.impactRange = { base, head };
    this.impactError = "";
    this.impactBusy = true;
    this.impactProgress = START;
    try {
      const res = await api.impact(this.project, base, head, (p) => {
        if (this.guard.isCurrent(req)) this.impactProgress = p;
      });
      if (!this.guard.isCurrent(req)) return; // superseded or cleared meanwhile
      this.setImpact(res, keepSelection);
    } catch (e) {
      if (this.guard.isCurrent(req)) this.impactError = `Impact failed: ${String(e)}`;
    } finally {
      if (this.guard.isCurrent(req)) {
        this.impactBusy = false;
        this.impactProgress = null;
      }
    }
  }

  clearImpact() {
    this.guard.invalidate();
    this.impactRange = null;
    this.historySel = null;
    this.impactError = "";
    this.impactBusy = false;
    this.impactProgress = null;
    this.clearDiff();
    this.setImpact(null);
  }

  /** Select history entry `sel`; its diff drives the impact. */
  pickHistory(sel: Selection) {
    if (!this.history || this.impactBusy) return;
    const d = selectionDiff(this.history.commits, sel);
    if (!d) return;
    this.historySel = sel;
    void this.loadDiff(d.base, d.head);
    void this.runImpact(d.base, d.head);
  }

  /** A typed diff is not a sidebar selection. */
  runTypedImpact(base: string | null, head: string | null) {
    if (this.impactBusy) return;
    this.historySel = null;
    void this.loadDiff(base, head);
    void this.runImpact(base, head);
  }

  /** Loads the diff of `base..head`; a later selection supersedes it. */
  async loadDiff(base: string | null, head: string | null): Promise<void> {
    if (this.multi) return;
    const req = this.diffGuard.begin();
    this.diff = { base, head, loading: true };
    try {
      const data = await api.diff(this.project, base, head);
      if (this.diffGuard.isCurrent(req)) this.diff = { base, head, loading: false, data };
    } catch (e) {
      if (this.diffGuard.isCurrent(req)) this.diff = { base, head, loading: false, error: String(e) };
    }
  }

  private clearDiff() {
    this.diffGuard.invalidate();
    this.diff = null;
  }

  /** With `keepSelection` (re-run after a rules reload), a selected cell that still exists
   * in the new matrix stays selected. */
  private setImpact(imp: Impact | null, keepSelection = false) {
    const before = this.matrix;
    const sel = this.selected;
    this.impact = imp;
    const now = this.matrix;
    if (now !== before) {
      // Another snapshot: cell ids are not comparable, unless only the rules changed.
      this.selected = keepSelection && sel && now ? retainSelection(sel, now) : null;
      void this.loadSymbols(false);
    }
  }
}

export const app = new AppState();
