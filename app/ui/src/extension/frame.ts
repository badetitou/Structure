// Detects an extension iframe that did not load (page CSP `frame-src`, blocked content...).
// The viewer posts READY to its parent as soon as it runs; the content script waits for it.

export const READY_TYPE = "structure:ready";

export function isReadyMessage(v: unknown): boolean {
  return typeof v === "object" && v !== null && (v as { type?: unknown }).type === READY_TYPE;
}

interface Msg {
  origin: string;
  data: unknown;
}
export interface FrameWatchEnv {
  addEventListener(type: "message", l: (e: Msg) => void): void;
  removeEventListener(type: "message", l: (e: Msg) => void): void;
  setTimeout(f: () => void, ms: number): unknown;
  clearTimeout(id: unknown): void;
}

/** Calls `onMissing` unless READY arrives from a page of `frameOrigin` (the extension's own
 * origin) within `delayMs`. The origin, not the window, identifies the sender: comparing window
 * objects is unreliable from a content script. Returns a cancel function. */
export function watchFrame(
  env: FrameWatchEnv,
  frameOrigin: string,
  onMissing: () => void,
  delayMs: number,
): () => void {
  const onMessage = (e: Msg) => {
    if (e.origin === frameOrigin && isReadyMessage(e.data)) stop();
  };
  const timer = env.setTimeout(() => {
    stop();
    onMissing();
  }, delayMs);
  function stop() {
    env.clearTimeout(timer);
    env.removeEventListener("message", onMessage);
  }
  env.addEventListener("message", onMessage);
  return stop;
}
