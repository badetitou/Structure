// Extension viewer page: runs the PR pipeline, then mounts the viewer on the result.
import "./extension.css";
import { mount } from "svelte";
import init, { PrAnalysis } from "@wasm/structure_wasm.js";
import wasmUrl from "@wasm/structure_wasm_bg.wasm?url";
import type { CellSymbols, WireImpact } from "../api";
import { setBackend } from "../api";
import { viewerBackend } from "../viewer/backend";
import Viewer from "../viewer/Viewer.svelte";
import { REQUIRED_ORIGINS, hasHostAccess, requestHostAccess } from "./access";
import { ext } from "./browser";
import { READY_TYPE } from "./frame";
import { parsePrParam } from "./github";
import { fetchPr } from "./pipeline";
import { TOKEN_KEY } from "./settings";

const target = document.getElementById("app") as HTMLElement;

function say(text: string, error = false) {
  target.textContent = text;
  target.style.cssText = `padding:1rem;font:14px system-ui;${error ? "color:#b42318" : ""}`;
}

/** Resolves once the GitHub hosts are accessible, asking the user (browser prompt) when they are not. */
async function ensureAccess(): Promise<void> {
  const perms = ext().permissions;
  if (await hasHostAccess(perms)) return;
  await new Promise<void>((resolve) => {
    target.textContent = "";
    target.style.cssText = "padding:1rem;font:14px system-ui;";
    const hosts = REQUIRED_ORIGINS.map((o) => o.replace("https://", "").replace("/*", "")).join(" and ");
    const p = document.createElement("p");
    p.textContent = `Structure needs access to ${hosts} to read the pull request. Your browser will ask you to confirm.`;
    const b = document.createElement("button");
    b.textContent = "Allow access to GitHub";
    b.style.cssText = "padding:6px 12px;cursor:pointer;";
    const note = document.createElement("p");
    b.addEventListener("click", () => {
      // permissions.request must be called from the click handler itself.
      void requestHostAccess(perms).then((ok) => {
        if (ok) resolve();
        else note.textContent = "Access was not granted. Allow it in the browser's add-on permissions to continue.";
      });
    });
    target.append(p, b, note);
  });
}

async function main() {
  // Tell the embedding page (content script) that this frame loaded.
  if (window.parent !== window) window.parent.postMessage({ type: READY_TYPE }, "https://github.com");

  const ref = parsePrParam(new URLSearchParams(location.search).get("pr"));
  if (!ref) return say("Missing or invalid ?pr=<owner>/<repo>/<number> parameter.", true);

  await ensureAccess();
  const stored = await ext().storage.local.get(TOKEN_KEY);
  const token = typeof stored[TOKEN_KEY] === "string" ? (stored[TOKEN_KEY] as string) : undefined;

  const [data] = await Promise.all([fetchPr(ref, { token, onProgress: say }), init({ module_or_path: wasmUrl })]);

  say("Analysing…");
  // Let the browser paint the message before the synchronous analysis.
  await new Promise((r) => setTimeout(r, 0));
  let analysis: PrAnalysis;
  try {
    analysis = new PrAnalysis(JSON.stringify(data.files), data.diff, `${ref.owner}/${ref.repo}`, data.base, data.head);
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    const unsupported = /no supported sources/.test(m);
    return say(
      unsupported ? "Unsupported language: no .java, .ts or .tsx source found in this repository." : `Analysis failed: ${m}`,
      true,
    );
  }
  const impact = JSON.parse(analysis.impact()) as WireImpact;
  setBackend(
    viewerBackend({
      impact,
      cellSymbols: async (cells, offset, limit) =>
        JSON.parse(analysis.cell_symbols(JSON.stringify(cells), offset, limit)) as CellSymbols,
    }),
  );
  target.textContent = "";
  target.removeAttribute("style");
  mount(Viewer, { target, props: { onError: (e) => e && say(e, true) } });
}

main().catch((e) => say(e instanceof Error ? e.message : String(e), true));
