import type { Impact, Matrix } from "./api";

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
