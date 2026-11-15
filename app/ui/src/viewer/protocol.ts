// postMessage protocol between a host page and the viewer.
//   host   -> viewer: { type: "structure:data", impact }
//   viewer -> host:   { type: "structure:cell-symbols", id, cells, offset, limit }
//   host   -> viewer: { type: "structure:cell-symbols:result", id, result }
import type { CellSymbols, WireImpact } from "../api";

export interface DataMessage {
  type: "structure:data";
  impact: WireImpact;
}
export interface CellSymbolsRequest {
  type: "structure:cell-symbols";
  id: number;
  cells: string[];
  offset: number;
  limit: number;
}
export interface CellSymbolsResult {
  type: "structure:cell-symbols:result";
  id: number;
  result: CellSymbols;
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

export function isDataMessage(v: unknown): v is DataMessage {
  return isObj(v) && v.type === "structure:data" && isObj(v.impact) && isObj(v.impact.matrix);
}
export function isCellSymbolsResult(v: unknown): v is CellSymbolsResult {
  return isObj(v) && v.type === "structure:cell-symbols:result" && typeof v.id === "number" && isObj(v.result);
}

/** Minimal slice of a window used to talk to the host. */
export interface Host {
  postMessage(message: unknown, targetOrigin: string): void;
}

/** Asks `host` for cell symbols; answers are fed to `onMessage`. */
export function cellSymbolsClient(host: Host, targetOrigin: string) {
  let next = 0;
  const pending = new Map<number, (r: CellSymbols) => void>();
  return {
    request(cells: string[], offset: number, limit: number): Promise<CellSymbols> {
      const id = ++next;
      return new Promise((resolve) => {
        pending.set(id, resolve);
        const msg: CellSymbolsRequest = { type: "structure:cell-symbols", id, cells, offset, limit };
        host.postMessage(msg, targetOrigin);
      });
    },
    onMessage(data: unknown): void {
      if (!isCellSymbolsResult(data)) return;
      pending.get(data.id)?.(data.result);
      pending.delete(data.id);
    },
  };
}
