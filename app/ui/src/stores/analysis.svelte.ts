import * as api from "../api";
import type { Matrix, Progress } from "../api";
import { analyzeAndRecord } from "../recent";

/** Progress shown before the first report. */
export const START: Progress = { step: "starting…", pct: 0 };

/** How often the project's rules files are checked for changes while a project is open. */
export const RULES_POLL_MS = 2000;

/** The analysis of the open project: its folders, the working-tree matrix and the rules reload. */
export class AnalysisStore {
  /** Folders analysed together (one for a single project). */
  paths = $state<string[]>([]);
  /** "loading" while analysing, "ready" once the working-tree matrix is there. */
  phase = $state<"idle" | "loading" | "ready">("idle");
  progress = $state<Progress>(START);
  /** Working-tree matrix. */
  work = $state.raw<Matrix | null>(null);
  /** Last rules reload failure (invalid rules file…); the matrix shown is kept. */
  rulesError = $state("");
  /** True while the rules are being reloaded. */
  rulesBusy = $state(false);
  /** Last seen stamp of the rules files, null until known. */
  private rulesStamp: string | null = null;
  private rulesReq = 0;
  private loadReq = 0;

  /** Identifies the current load: answers obtained under another id are late and dropped. */
  get loadId(): number {
    return this.loadReq;
  }
  /** First folder: the project for single-project code. */
  get project(): string {
    return this.paths[0] ?? "";
  }
  /** Several distinct projects analysed together (per the loaded matrix, as the backend dedups roots): git panels are unavailable. */
  get multi(): boolean {
    return (this.work?.projects?.length ?? 0) > 1;
  }
  /** The rules can be reloaded (and are watched) once the analysis is there. */
  get rulesReloadable(): boolean {
    return api.capabilities().reclassify && this.phase === "ready";
  }

  /** Back to the empty state; late answers of a previous load or rules reload are ignored. */
  reset() {
    this.loadReq++;
    this.paths = [];
    this.phase = "idle";
    this.progress = START;
    this.work = null;
    this.rulesReq++;
    this.rulesError = "";
    this.rulesBusy = false;
    this.rulesStamp = null;
  }

  /** Analyses `paths` (progress in `progress`) under the current load id. Resolves true once
   * the matrix is there, false when another load or a reset superseded it; rejects when the
   * analysis fails. */
  async load(paths: string[]): Promise<boolean> {
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
      if (req !== this.loadReq) return false; // superseded: not this load's failure to report
      throw e;
    }
    if (req !== this.loadReq) return false;
    this.work = m;
    this.phase = "ready";
    void stamp.then((s) => {
      if (req === this.loadReq && this.rulesStamp === null) this.rulesStamp = s;
    });
    return true;
  }

  /** Stamp of the rules files of `paths`, null when unknown (unsupported or failed). */
  private async readRulesStamp(paths: string[]): Promise<string | null> {
    if (!api.capabilities().reclassify) return null;
    try {
      return (await api.rulesStamp(paths)) ?? null;
    } catch {
      return null;
    }
  }

  /** Re-applies the project's rules to the analysis without re-parsing it. `apply` installs
   * the reclassified matrix (it sets `work`, see `session.ts`); on failure the matrix stays
   * and the error goes to `rulesError`. */
  async reloadRules(apply: (m: Matrix) => void): Promise<void> {
    if (!this.rulesReloadable || !this.work) return;
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
      apply(m);
    } catch (e) {
      if (current()) this.rulesError = `Rules reload failed: ${String(e)}`;
    } finally {
      if (current()) this.rulesBusy = false;
    }
  }

  /** Reloads the rules (through `apply`) when their files changed since last checked. */
  async checkRules(apply: (m: Matrix) => void): Promise<void> {
    if (!this.rulesReloadable || this.rulesBusy) return;
    const load = this.loadReq;
    const stamp = await this.readRulesStamp(this.paths);
    if (load !== this.loadReq || stamp === null) return;
    const prev = this.rulesStamp;
    this.rulesStamp = stamp;
    if (prev !== null && prev !== stamp) await this.reloadRules(apply);
  }

  /** Polls the rules files every `RULES_POLL_MS`; returns the function that stops it.
   * Nothing to poll when the backend cannot reload the rules. */
  watchRules(apply: (m: Matrix) => void): () => void {
    if (!api.capabilities().reclassify) return () => {};
    const id = setInterval(() => void this.checkRules(apply), RULES_POLL_MS);
    return () => clearInterval(id);
  }
}

export const analysis = new AnalysisStore();
