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
