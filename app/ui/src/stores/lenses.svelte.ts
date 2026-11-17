import { SvelteMap } from "svelte/reactivity";
import * as api from "../api";
import type { Impact, LensCells, LensInfo, Matrix } from "../api";
import { DEFAULT_LENS, liveSnapshots, snapshotId, withLensCells } from "../lens";
import { analysis } from "./analysis.svelte";

/** One answer of the backend: loading, there, or failed. */
type Entry<T> = { state: "loading" } | { state: "ok"; value: T } | { state: "error"; error: string };

/**
 * The lenses offered and what the views need of a non-default one, fetched on first read and
 * cached: the matrix of the analysis shown through the lens (by lens, commit and generation, so
 * a reclassified analysis is fetched again), and each impact's cells in the lens (by impact,
 * computed by the backend from the impact's symbols). The default lens needs nothing: its
 * matrix and cells are the ones already loaded. Reads never fetch synchronously: a missing
 * entry is requested in a microtask, so reading from a derived value is safe.
 *
 * Bounded by `prune` (entries no view can show any more are dropped); a failed entry stays
 * until `retry` (reads never re-request it, so a lasting failure is not fetched in a loop).
 */
export class LensStore {
  /** The lenses the backend reported once a project was loaded; null until then. */
  private reported = $state.raw<readonly LensInfo[] | null>(null);
  /** The backend answered (or failed to answer) `refresh`: `available` is its list. */
  private answered = $state(false);
  /** Lens matrices by `${lens}\0${snapshotId}`. */
  private matrices = new SvelteMap<string, Entry<Matrix>>();
  /** Impact cells by `${lens}\0${impact id}`. */
  private cells = new SvelteMap<string, Entry<LensCells>>();
  /** Lensed impacts by impact, then lens, with the entry they were made from: a view gets the
   * same object on every read, and an impact no longer referenced is collected with them. */
  private lensed = new WeakMap<Impact, Map<string, { entry: Entry<LensCells> | undefined; out: Impact }>>();
  private impactIds = new WeakMap<Impact, number>();
  private nextImpact = 1;
  /** Bumped by `reset`: answers to an older epoch are dropped. */
  private epoch = 0;

  /** The lenses offered, the default first. */
  get available(): readonly LensInfo[] {
    return this.reported ?? api.capabilities().lenses;
  }

  /** The backend's list is in (see `resolveLens`); a non-default lens is not drawn before. */
  get known(): boolean {
    return this.answered;
  }

  /** Asks the backend for its lenses (once per project load); keeps the known ones on failure. */
  async refresh(): Promise<void> {
    const epoch = this.epoch;
    try {
      const list = await api.lenses();
      if (epoch === this.epoch && list.length > 0) this.reported = list;
    } catch {
      // Keep `capabilities.lenses`.
    }
    if (epoch === this.epoch) this.answered = true;
  }

  /** A new project: nothing cached. The lenses reported stay (same backend). */
  reset() {
    this.epoch++;
    this.matrices.clear();
    this.cells.clear();
    this.lensed = new WeakMap();
  }

  /** Drops what no view can show any more: lens matrices of analyses other than `work` (the
   * working tree's) and the commit snapshots of `impacts` (the overlays listed), and the cells
   * of impacts not in `impacts`. Call it outside derived values (it writes reactive maps). */
  prune(work: Matrix | null, impacts: readonly Impact[]) {
    const live = liveSnapshots(work, impacts);
    for (const key of [...this.matrices.keys()]) {
      if (!live.has(key.slice(key.indexOf("\u0000") + 1))) this.matrices.delete(key);
    }
    const ids = new Set(impacts.map((i) => this.impactIds.get(i)).filter((id) => id !== undefined).map(String));
    for (const key of [...this.cells.keys()]) {
      if (!ids.has(key.slice(key.lastIndexOf("\u0000") + 1))) this.cells.delete(key);
    }
  }

  /** Forgets the failed entries, so the next read asks the backend again. */
  retry() {
    dropErrors(this.matrices);
    dropErrors(this.cells);
  }

  /** `base` (the matrix shown in the default lens, of commit `head`) through `lens`; null while
   * it loads (or the backend's lenses are not known yet) or when it failed (see `matrixError`). */
  matrix(lens: string, base: Matrix | null, head: string | null): Matrix | null {
    if (!base || lens === DEFAULT_LENS) return base;
    const e = this.matrixEntry(lens, base, head);
    return e?.state === "ok" ? e.value : null;
  }

  /** Why `matrix` gives null for these arguments ("" while loading or when fine). */
  matrixError(lens: string, base: Matrix | null, head: string | null): string {
    if (!base || lens === DEFAULT_LENS) return "";
    const e = this.matrixEntry(lens, base, head);
    return e?.state === "error" ? e.error : "";
  }

  private matrixEntry(lens: string, base: Matrix, head: string | null): Entry<Matrix> | undefined {
    if (!this.answered) return undefined;
    const generation = base.generation ?? 0;
    const key = `${lens}\u0000${snapshotId(head, generation)}`;
    const e = this.matrices.get(key);
    if (!e) {
      const paths = analysis.paths;
      this.request(this.matrices, key, () => api.matrix(paths, head, generation, lens));
    }
    return e;
  }

  /** `imp` with its cells in `lens` (itself for the default lens); while they load, or when
   * they failed (see `impactError`), an impact with no cells (nothing drawn rather than another
   * lens' ids). */
  impact(lens: string, imp: Impact): Impact {
    if (lens === DEFAULT_LENS) return imp;
    const e = this.cellsEntry(lens, imp);
    let byLens = this.lensed.get(imp);
    if (!byLens) {
      byLens = new Map();
      this.lensed.set(imp, byLens);
    }
    const prev = byLens.get(lens);
    if (prev && prev.entry === e) return prev.out;
    // A new loading entry draws the same as none: keep the object.
    if (prev && prev.entry?.state !== "ok" && e?.state !== "ok") return prev.out;
    const out = withLensCells(imp, e?.state === "ok" ? e.value : null);
    byLens.set(lens, { entry: e, out });
    return out;
  }

  /** Why `impact` gives no cells for `imp` in `lens` ("" while loading or when fine). */
  impactError(lens: string, imp: Impact): string {
    if (lens === DEFAULT_LENS) return "";
    const e = this.cellsEntry(lens, imp);
    return e?.state === "error" ? e.error : "";
  }

  private cellsEntry(lens: string, imp: Impact): Entry<LensCells> | undefined {
    if (!this.answered) return undefined;
    let id = this.impactIds.get(imp);
    if (id === undefined) {
      id = this.nextImpact++;
      this.impactIds.set(imp, id);
    }
    const key = `${lens}\u0000${id}`;
    const e = this.cells.get(key);
    if (!e) {
      const project = analysis.project;
      this.request(this.cells, key, () =>
        api.impactCells(project, imp.head, lens, imp.changed_symbols, imp.impacted_symbols),
      );
    }
    return e;
  }

  /** Fetches `key` into `map` in a microtask (never while a derived value is computed). The
   * answer is dropped when the entry was pruned or retried meanwhile. */
  private request<T>(map: SvelteMap<string, Entry<T>>, key: string, fetch: () => Promise<T>) {
    const epoch = this.epoch;
    queueMicrotask(() => {
      if (epoch !== this.epoch || map.has(key)) return;
      const loading: Entry<T> = { state: "loading" };
      map.set(key, loading);
      const settle = (e: Entry<T>) => {
        if (epoch === this.epoch && map.get(key) === loading) map.set(key, e);
      };
      fetch().then(
        (value) => settle({ state: "ok", value }),
        (e: unknown) => settle({ state: "error", error: String(e) }),
      );
    });
  }
}

function dropErrors<T>(map: SvelteMap<string, Entry<T>>) {
  for (const [key, e] of [...map.entries()]) if (e.state === "error") map.delete(key);
}

export const lenses = new LensStore();
