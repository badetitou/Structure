import * as api from "../api";
import type { Impact, Progress } from "../api";
import { RequestGuard } from "../matrixsel";
import { START } from "./progress";

/** A git range; null stands for HEAD / the working tree. */
export interface Range {
  base: string | null;
  head: string | null;
}

/** One highlight of the matrix: the impact of a git range, with its own run state.
 * Self-contained (no other store is read here) so that several can later be listed side by side. */
export class ImpactOverlay {
  /** The impact shown, null when none. */
  current = $state.raw<Impact | null>(null);
  /** True while an impact runs (it may analyse a commit snapshot first); new runs are ignored meanwhile. */
  busy = $state(false);
  /** Progress of the running impact, null when none runs. */
  progress = $state<Progress | null>(null);
  error = $state("");
  /** Range of the impact shown (or running), null once cleared. */
  range: Range | null = null;
  private guard = new RequestGuard();

  /** Computes the impact of `base..head` in `project` and hands it to `apply` (which sets
   * `current`, see `session.ts`). Ignored while another run is busy; a later run, `clear`
   * or `abandon` makes this one's answer stale. */
  async run(project: string, range: Range, apply: (imp: Impact) => void): Promise<void> {
    if (this.busy) return;
    const req = this.guard.begin();
    this.range = range;
    this.error = "";
    this.busy = true;
    this.progress = START;
    try {
      const res = await api.impact(project, range.base, range.head, (p) => {
        if (this.guard.isCurrent(req)) this.progress = p;
      });
      if (!this.guard.isCurrent(req)) return; // superseded or cleared meanwhile
      apply(res);
    } catch (e) {
      if (this.guard.isCurrent(req)) this.error = `Impact failed: ${String(e)}`;
    } finally {
      if (this.guard.isCurrent(req)) {
        this.busy = false;
        this.progress = null;
      }
    }
  }

  /** Drops the running request (its answer will be ignored) so that a new one can start; the impact shown and its range stay. */
  abandon() {
    this.guard.invalidate();
    this.busy = false;
  }

  /** No impact: the shown one, the running one and the last error are dropped. */
  clear() {
    this.guard.invalidate();
    this.range = null;
    this.current = null;
    this.error = "";
    this.busy = false;
    this.progress = null;
  }
}

export const impact = new ImpactOverlay();
