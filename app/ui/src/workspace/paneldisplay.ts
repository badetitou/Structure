// Display options owned by a panel instance (a matrix panel), kept in its dockview params
// (`params.display`) so they are saved with the layout, and the rules that pick the panel the
// toolbar acts on. Pure helpers: the reactive side is `displays.svelte.ts`.
import type { Mode } from "../layout";
import { parseLens } from "../lens";
import { parseMode } from "../mode";

/** How one matrix panel is drawn, and through which lens (the backend's quotient, see
 * `lens.ts`). Angled labels are an app-wide preference, not part of it. */
export interface PanelDisplay {
  mode: Mode;
  transposed: boolean;
  aligned: boolean;
  showHttp: boolean;
  showRpc: boolean;
  /** Lens id; the default lens unless the panel picked another. */
  lens: string;
}

/** Copy of the display fields of `d` (drops anything else). */
export function snapshot(d: PanelDisplay): PanelDisplay {
  return {
    mode: d.mode,
    transposed: d.transposed,
    aligned: d.aligned,
    showHttp: d.showHttp,
    showRpc: d.showRpc,
    lens: d.lens,
  };
}

const bool = (v: unknown, fallback: boolean) => (typeof v === "boolean" ? v : fallback);

/**
 * The display stored in a panel's params, each field checked on its own: a missing or
 * malformed field takes `fallback`'s. `stored` is false when the params hold no display
 * object at all (a panel saved before options were per panel, or a new one): it then adopts
 * `fallback` whole, the global defaults.
 */
export function parseDisplay(params: unknown, fallback: PanelDisplay): { display: PanelDisplay; stored: boolean } {
  const raw = typeof params === "object" && params !== null ? (params as Record<string, unknown>).display : undefined;
  if (typeof raw !== "object" || raw === null) return { display: snapshot(fallback), stored: false };
  const o = raw as Record<string, unknown>;
  return {
    display: {
      mode: typeof o.mode === "string" ? parseMode(o.mode) : fallback.mode,
      transposed: bool(o.transposed, fallback.transposed),
      aligned: bool(o.aligned, fallback.aligned),
      showHttp: bool(o.showHttp, fallback.showHttp),
      showRpc: bool(o.showRpc, fallback.showRpc),
      lens: o.lens === undefined ? fallback.lens : parseLens(o.lens),
    },
    stored: true,
  };
}

/** The params patch that stores `d` in a panel. */
export function displayParams(d: PanelDisplay): { display: PanelDisplay } {
  return { display: snapshot(d) };
}

/** `recent` (most recently focused first) with `id` moved to the front. */
export function touch(recent: readonly string[], id: string): string[] {
  return [id, ...recent.filter((r) => r !== id)];
}

/** `recent` with `id` appended when absent (a panel that appears without focus). */
export function enlist(recent: readonly string[], id: string): string[] {
  return recent.includes(id) ? [...recent] : [...recent, id];
}

/** `recent` without `id` (a closed panel). */
export function forget(recent: readonly string[], id: string): string[] {
  return recent.filter((r) => r !== id);
}

/** The panel the toolbar acts on: the focused one, else the last focused still open; null when none. */
export function activeOf(recent: readonly string[]): string | null {
  return recent[0] ?? null;
}

/**
 * Id of a new instance of contribution `base`: `base-N` with the smallest N >= 2 not taken
 * (the first instance keeps the bare id, as single-instance panels do).
 */
export function nextInstanceId(base: string, taken: { has(id: string): boolean }): { id: string; n: number } {
  let n = 2;
  while (taken.has(`${base}-${n}`)) n++;
  return { id: `${base}-${n}`, n };
}
