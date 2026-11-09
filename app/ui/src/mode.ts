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
