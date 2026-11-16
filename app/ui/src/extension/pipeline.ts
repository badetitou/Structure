// Fetch-based GitHub pipeline: PR metadata, diff and head sources. No `chrome.*`.
// Plain TS so Node (type stripping) can import it from scripts/try-pr.mjs.

import { PrError, apiHeaders, githubError, prUrl, tarballUrl } from "./github.ts";
import type { PrRef } from "./github.ts";
import { readTar } from "./tar.ts";
import type { TarFile } from "./tar.ts";

export interface PrData {
  base: string;
  head: string;
  diff: string;
  files: TarFile[];
}

export interface FetchOptions {
  token?: string;
  fetch?: typeof fetch;
  onProgress?: (text: string) => void;
}

export const SOURCE_EXTENSIONS = [".java", ".ts", ".tsx", ".st"];

export async function fetchPr(ref: PrRef, opts: FetchOptions = {}): Promise<PrData> {
  const f = opts.fetch ?? fetch;
  const say = opts.onProgress ?? (() => {});
  // The token is attached here, to api.github.com URLs only; the redirect to codeload gets none from us.
  const get = async (url: string, accept: string, what: string): Promise<Response> => {
    let res: Response;
    try {
      res = await f(url, { headers: apiHeaders(opts.token, accept) });
    } catch (e) {
      throw new PrError("network", `Network error while fetching the ${what}: ${String(e)}`);
    }
    if (!res.ok) throw githubError(res, what);
    return res;
  };

  say("Fetching pull request…");
  const pr = (await (await get(prUrl(ref), "application/vnd.github+json", "pr")).json()) as {
    base?: { sha?: string };
    head?: { sha?: string };
  };
  const base = pr.base?.sha;
  const head = pr.head?.sha;
  if (!base || !head) throw new PrError("http", "Unexpected GitHub response: no base/head sha.");

  say("Fetching diff…");
  const diff = await (await get(prUrl(ref), "application/vnd.github.diff", "diff")).text();

  say("Downloading sources…");
  const tar = await get(tarballUrl(ref, head), "application/vnd.github+json", "sources");
  if (!tar.body) throw new PrError("http", "GitHub returned an empty archive.");
  const stream = tar.body.pipeThrough(new DecompressionStream("gzip")) as ReadableStream<Uint8Array>;
  say("Reading archive…");
  const files = await readTar(stream, { extensions: SOURCE_EXTENSIONS });
  say(`Read ${files.length} source files`);
  return { base, head, diff, files };
}
