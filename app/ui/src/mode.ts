import type { Mode } from "./layout";

export const MODE_KEY = "structure.matrix.mode";

/** Stored value → mode; anything unknown falls back to the normal view. */
export function parseMode(raw: string | null): Mode {
  return raw === "dense" ? "dense" : "normal";
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
