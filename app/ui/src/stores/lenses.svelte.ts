import { SvelteMap } from "svelte/reactivity";
import * as api from "../api";
import type { Impact, LensCells, LensInfo, Matrix } from "../api";
import { DEFAULT_LENS, withLensCells } from "../lens";
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
 */
export class LensStore {
  /** The lenses the backend reported once a project was loaded; null until then. */
  private reported = $state.raw<readonly LensInfo[] | null>(null);
  private matrices = new SvelteMap<string, Entry<Matrix>>();
  private cells = new SvelteMap<string, Entry<LensCells>>();
  /** Lensed impacts by key, so a view gets the same object on every read. */
  private lensed = new Map<string, Impact>();
  private impactIds = new WeakMap<Impact, number>();
  private nextImpact = 1;
  /** Bumped by `reset`: answers to an older epoch are dropped. */
  private epoch = 0;

  /** The lenses offered, the default first. */
  get available(): readonly LensInfo[] {
    return this.reported ?? api.capabilities().lenses;
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
  }

  /** A new project: nothing cached. The lenses reported stay (same backend). */
  reset() {
    this.epoch++;
    this.matrices.clear();
    this.cells.clear();
    this.lensed.clear();
  }

  /** `base` (the matrix shown in the default lens, of commit `head`) through `lens`; null while
   * it loads or when it failed (see `matrixError`). */
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
    const generation = base.generation ?? 0;
    const key = `${lens}\u0000${head ?? ""}\u0000${generation}`;
    const e = this.matrices.get(key);
    if (!e) {
      const paths = analysis.paths;
      this.request(this.matrices, key, () => api.matrix(paths, head, generation, lens));
    }
    return e;
  }

  /** `imp` with its cells in `lens` (itself for the default lens); while they load, or when
   * they failed, an impact with no cells (nothing drawn rather than another lens' ids). */
  impact(lens: string, imp: Impact): Impact {
    if (lens === DEFAULT_LENS) return imp;
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
    const state = e?.state ?? "loading";
    const lkey = `${key}\u0000${state}`;
    let out = this.lensed.get(lkey);
    if (!out) {
      out = withLensCells(imp, e?.state === "ok" ? e.value : null);
      this.lensed.set(lkey, out);
    }
    return out;
  }

  /** Fetches `key` into `map` in a microtask (never while a derived value is computed). */
  private request<T>(map: SvelteMap<string, Entry<T>>, key: string, fetch: () => Promise<T>) {
    const epoch = this.epoch;
    queueMicrotask(() => {
      if (epoch !== this.epoch || map.has(key)) return;
      map.set(key, { state: "loading" });
      fetch().then(
        (value) => {
          if (epoch === this.epoch) map.set(key, { state: "ok", value });
        },
        (e: unknown) => {
          if (epoch === this.epoch) map.set(key, { state: "error", error: String(e) });
        },
      );
    });
  }
}

export const lenses = new LensStore();
