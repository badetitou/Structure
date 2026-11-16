import type { Impact, Matrix } from "./api";
import type { ViewCell } from "./domaintree";

/** The matrix to display: the commit snapshot's when the impact carries one,
 * otherwise the working-tree matrix. */
export function shownMatrix(work: Matrix, impact: Impact | null): Matrix {
  return impact?.matrix ?? work;
}

/** Monotonic request ids: only the latest `begin()` is current, `invalidate()`
 * (Clear) makes every pending request stale. */
export class RequestGuard {
  private id = 0;
  begin(): number {
    return ++this.id;
  }
  invalidate() {
    this.id++;
  }
  isCurrent(id: number): boolean {
    return id === this.id;
  }
}

/** The selected view cell carried over to `m` (after the rules were reloaded): its member
 * cells that still exist, with their new total size; null when none is left. The matrix
 * view then swaps it for its own cell of the same id, or drops it (`syncSelection`). */
export function retainSelection(sel: ViewCell, m: Matrix): ViewCell | null {
  const sizes = new Map(m.cells.map((c) => [c.id, c.size]));
  const members = sel.members.filter((id) => sizes.has(id));
  if (members.length === 0) return null;
  const size = members.reduce((n, id) => n + (sizes.get(id) ?? 0), 0);
  return { ...sel, members, size };
}
