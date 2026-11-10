import type { Mode } from "./layout";

export const MODE_KEY = "structure.matrix.mode";

/** Stored value → mode; anything unknown falls back to the normal view. */
export function parseMode(raw: string | null): Mode {
  return raw === "dense" || raw === "flame" ? raw : "normal";
}

export function loadMode(): Mode {
  try {
    return parseMode(localStorage.getItem(MODE_KEY));
  } catch {
    return "normal";
  }
}

export function saveMode(mode: Mode) {
  try {
    localStorage.setItem(MODE_KEY, mode);
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}

export const HTTP_KEY = "structure.httpEdges";

/** Stored value -> toggle; on unless explicitly "false". */
export function parseShowHttp(raw: string | null): boolean {
  return raw !== "false";
}

export function loadShowHttp(): boolean {
  try {
    return parseShowHttp(localStorage.getItem(HTTP_KEY));
  } catch {
    return true;
  }
}

export function saveShowHttp(on: boolean) {
  try {
    localStorage.setItem(HTTP_KEY, String(on));
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}

export const RPC_KEY = "structure.rpcEdges";

export function parseShowRpc(raw: string | null): boolean {
  return raw !== "false";
}

export function loadShowRpc(): boolean {
  try {
    return parseShowRpc(localStorage.getItem(RPC_KEY));
  } catch {
    return true;
  }
}

export function saveShowRpc(on: boolean) {
  try {
    localStorage.setItem(RPC_KEY, String(on));
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}

export const TRANSPOSED_KEY = "structure.matrix.transposed";

/** Whether the matrix shows domains as rows; off when unset or storage is unavailable. */
export function loadTransposed(): boolean {
  try {
    return localStorage.getItem(TRANSPOSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function saveTransposed(on: boolean) {
  try {
    localStorage.setItem(TRANSPOSED_KEY, on ? "1" : "0");
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}

export const ALIGNED_KEY = "structure.matrix.aligned";

/** Stored value -> toggle; on unless explicitly "false". */
export function parseAligned(raw: string | null): boolean {
  return raw !== "false";
}

export function loadAligned(): boolean {
  try {
    return parseAligned(localStorage.getItem(ALIGNED_KEY));
  } catch {
    return true;
  }
}

export function saveAligned(on: boolean) {
  try {
    localStorage.setItem(ALIGNED_KEY, String(on));
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}

export const ANGLED_KEY = "structure.options.angledLabels";

/** Whether rotated top labels are drawn at 45 degrees; off when unset or storage is unavailable. */
export function loadAngled(): boolean {
  try {
    return localStorage.getItem(ANGLED_KEY) === "1";
  } catch {
    return false;
  }
}

export function saveAngled(on: boolean) {
  try {
    localStorage.setItem(ANGLED_KEY, on ? "1" : "0");
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}
