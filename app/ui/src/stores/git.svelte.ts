import * as api from "../api";
import type { Diff, History } from "../api";
import type { Selection } from "../history";
import { RequestGuard } from "../matrixsel";
import { analysis } from "./analysis.svelte";

/** The diff panel's content: the requested range and its diff once loaded. */
export interface DiffState {
  base: string | null;
  head: string | null;
  loading: boolean;
  data?: Diff;
  error?: string;
}

/** Git side of a single project: its history, the history selection and the diff shown. */
export class GitStore {
  /** Null when unavailable (not a git repo, error): the sidebar is hidden. */
  history = $state.raw<History | null>(null);
  historySel = $state.raw<Selection | null>(null);
  /** Textual diff of the history selection (or typed range), null when none is selected. */
  diff = $state.raw<DiffState | null>(null);
  private diffGuard = new RequestGuard();

  /** History panel: the backend has a history and this single project has one (a git repo). */
  get historyAvailable(): boolean {
    return api.capabilities().history && !analysis.multi && this.history !== null;
  }
  /** Diff panel: the backend computes diffs of the history's ranges. */
  get diffAvailable(): boolean {
    return api.capabilities().diff && this.historyAvailable;
  }

  reset() {
    this.history = null;
    this.historySel = null;
    this.clearDiff();
  }

  /** Loads the history of `project` when the backend has one; kept only while `isCurrent()`. Never rejects. */
  async loadHistory(project: string, isCurrent: () => boolean): Promise<void> {
    if (!api.capabilities().history) return;
    try {
      const h = await api.history(project);
      if (isCurrent()) this.history = h;
    } catch {
      // history is optional
    }
  }

  /** Loads the diff of `base..head`; a later selection supersedes it. */
  async loadDiff(base: string | null, head: string | null): Promise<void> {
    if (analysis.multi || !api.capabilities().diff) return;
    const req = this.diffGuard.begin();
    this.diff = { base, head, loading: true };
    try {
      const data = await api.diff(analysis.project, base, head);
      if (this.diffGuard.isCurrent(req)) this.diff = { base, head, loading: false, data };
    } catch (e) {
      if (this.diffGuard.isCurrent(req)) this.diff = { base, head, loading: false, error: String(e) };
    }
  }

  clearDiff() {
    this.diffGuard.invalidate();
    this.diff = null;
  }
}

export const git = new GitStore();
