import * as api from "../api";
import type { Impact, Progress } from "../api";
import type { Selection } from "../history";
import { RequestGuard } from "../matrixsel";
import { addOverlay, clickSelection, selectionAfterRemoval } from "../overlays";
import type { Overlay, Range } from "../overlays";
import { START } from "./progress";

export type { Overlay, Range } from "../overlays";

/** The highlights of the matrix: a list of named overlays (impacts of git ranges, see
 * `overlays.ts` for the list and selection rules), the selected ones, and the run state of the
 * impact being computed. Self-contained (no other store is read here). Session data: nothing persists. */
export class ImpactOverlays {
  /** Loaded overlays, oldest first. */
  overlays = $state.raw<Overlay[]>([]);
  /** Ids of the overlays shown, in the order they were selected; empty when none. */
  selectedIds = $state.raw<string[]>([]);
  /** True while an impact runs (it may analyse a commit snapshot first); new runs are ignored meanwhile. */
  busy = $state(false);
  /** Progress of the running impact, null when none runs. */
  progress = $state<Progress | null>(null);
  error = $state("");
  /** Range of the running (or abandoned) request, null once it settled. */
  private pending: Range | null = null;
  private guard = new RequestGuard();
  private nextId = 1;

  /** Id of the first selected overlay (the one the diff and history follow), null when none. */
  get selectedId(): string | null {
    return this.selectedIds[0] ?? null;
  }
  /** The first selected overlay, null when none. */
  get selected(): Overlay | null {
    return this.overlays.find((o) => o.id === this.selectedId) ?? null;
  }
  /** The selected overlays, in list order (their bands follow it). */
  get selectedOverlays(): Overlay[] {
    return this.overlays.filter((o) => this.selectedIds.includes(o.id));
  }
  /** Several overlays are drawn together (on the working-tree matrix). */
  get multi(): boolean {
    return this.selectedIds.length >= 2;
  }
  /** The impact drawn alone (the single selected overlay's); null when none or several are shown. */
  get current(): Impact | null {
    return this.multi ? null : (this.selected?.impact ?? null);
  }
  /** Range of the impact running, else of the first one shown; null when none. */
  get range(): Range | null {
    return this.pending ?? this.selected?.range ?? null;
  }

  /** Computes the impact of `base..head` in `project` and hands it to `apply` (which adds it,
   * see `session.ts`). Ignored while another run is busy; a later run, `clear` or `abandon`
   * makes this one's answer stale. */
  async run(project: string, range: Range, apply: (imp: Impact) => void): Promise<void> {
    if (this.busy) return;
    const req = this.guard.begin();
    this.pending = range;
    this.error = "";
    this.busy = true;
    this.progress = START;
    try {
      const res = await api.impact(project, range.base, range.head, (p) => {
        if (this.guard.isCurrent(req)) this.progress = p;
      });
      if (!this.guard.isCurrent(req)) return; // superseded or cleared meanwhile
      this.pending = null;
      apply(res);
    } catch (e) {
      if (this.guard.isCurrent(req)) this.error = `Impact failed: ${String(e)}`;
    } finally {
      if (this.guard.isCurrent(req)) {
        this.pending = null;
        this.busy = false;
        this.progress = null;
      }
    }
  }

  /** Adds `impact` of `range` to the list (see `addOverlay`) and selects it alone, unless it
   * updated an overlay already selected (a re-run): then the selection stays. */
  add(range: Range, impact: Impact, historySel: Selection | null = null) {
    const { list, id } = addOverlay(this.overlays, range, impact, historySel, `o${this.nextId}`);
    if (id === `o${this.nextId}`) this.nextId++;
    this.overlays = list;
    if (!this.selectedIds.includes(id)) this.selectedIds = [id];
  }

  /** Shows overlay `id` alone, or with `toggle` adds it to the overlays shown or takes it out
   * (see `clickSelection`). Ignored when unknown. */
  select(id: string, toggle = false) {
    if (this.overlays.some((o) => o.id === id)) this.selectedIds = clickSelection(this.selectedIds, id, toggle);
  }

  /** Keeps overlay `id` in the list when the next impact runs. */
  pin(id: string) {
    this.overlays = this.overlays.map((o) => (o.id === id ? { ...o, pinned: true } : o));
  }

  /** Drops overlay `id`; when it was the only one shown, the most recent one left is shown instead. */
  remove(id: string) {
    this.overlays = this.overlays.filter((o) => o.id !== id);
    this.selectedIds = selectionAfterRemoval(this.overlays, this.selectedIds, id);
  }

  /** Every overlay (or those of `ids`) must be computed again: with rules that changed since,
   * or because the backend no longer holds the analysis it was computed on. */
  markStale(ids?: readonly string[]) {
    this.overlays = this.overlays.map((o) => (!ids || ids.includes(o.id) ? { ...o, stale: true } : o));
  }

  /** Drops the running request (its answer will be ignored) so that a new one can start; the overlays and the range stay. */
  abandon() {
    this.guard.invalidate();
    this.busy = false;
  }

  /** No impact shown: the selection, the unpinned overlay, the running request and the last
   * error are dropped; pinned overlays stay in the list. */
  clear() {
    this.guard.invalidate();
    this.pending = null;
    this.overlays = this.overlays.filter((o) => o.pinned);
    this.selectedIds = [];
    this.error = "";
    this.busy = false;
    this.progress = null;
  }

  /** Back to an empty list (a new project). */
  reset() {
    this.clear();
    this.overlays = [];
  }
}

export const impact = new ImpactOverlays();
