import "../app.css";
import { mount } from "svelte";
import type { CellSymbol, WireImpact, WireMatrix } from "../api";
import { setBackend } from "../api";
import { viewerBackend } from "./backend";
import type { ViewerData } from "./backend";
import { cellSymbolsClient, isDataMessage } from "./protocol";
import Viewer from "./Viewer.svelte";

const target = document.getElementById("app") as HTMLElement;

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return (await res.json()) as T;
}

/** `?sample=1`: the sample impact + matrix, with symbols answered locally. */
async function sampleData(): Promise<ViewerData> {
  const [impact, matrix, all] = await Promise.all([
    getJson<WireImpact>("/samples/impact.json"),
    getJson<WireMatrix>("/samples/matrix.json"),
    getJson<Record<string, CellSymbol[]>>("/samples/cell-symbols.json"),
  ]);
  return {
    impact: { ...impact, matrix },
    async cellSymbols(cells, offset, limit) {
      const list = cells.flatMap((c) => all[c] ?? []);
      return { total: list.length, offset, symbols: list.slice(offset, offset + limit) };
    },
  };
}

/** Waits for `{ type: "structure:data", impact }` from the parent or opener. */
function hostData(): Promise<ViewerData> {
  const host = window.parent !== window ? window.parent : window.opener;
  return new Promise((resolve) => {
    let client: ReturnType<typeof cellSymbolsClient> | null = null;
    window.addEventListener("message", (e: MessageEvent) => {
      if (!host || e.source !== host) return;
      if (isDataMessage(e.data) && !client) {
        const c = cellSymbolsClient(host, e.origin === "null" ? "*" : e.origin);
        client = c;
        resolve({ impact: e.data.impact, cellSymbols: (cells, offset, limit) => c.request(cells, offset, limit) });
      } else {
        client?.onMessage(e.data);
      }
    });
  });
}

function say(text: string) {
  target.textContent = text;
  target.style.cssText = "padding:1rem;font:14px system-ui";
}

async function main() {
  say("Waiting for data…");
  const data = new URLSearchParams(location.search).has("sample") ? await sampleData() : await hostData();
  setBackend(viewerBackend(data));
  target.textContent = "";
  target.removeAttribute("style");
  mount(Viewer, { target, props: { onError: (e) => e && say(e) } });
}

main().catch((e) => say(`Viewer failed: ${String(e)}`));
