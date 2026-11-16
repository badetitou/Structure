import { describe, expect, it } from "vitest";
import { PrError, githubError, parsePrParam, parsePrPath, parsePrShorthand } from "./github";
import { fetchPr } from "./pipeline";
import { readTar } from "./tar";

const enc = new TextEncoder();

function octal(n: number, len: number): string {
  return n.toString(8).padStart(len - 1, "0") + "\0";
}

function header(name: string, size: number, type: string, prefix = ""): Uint8Array {
  const h = new Uint8Array(512);
  h.set(enc.encode(name).subarray(0, 100), 0);
  h.set(enc.encode(octal(0o644, 8)), 100);
  h.set(enc.encode(octal(size, 12)), 124);
  h.set(enc.encode(type), 156);
  h.set(enc.encode("ustar\0" + "00"), 257);
  h.set(enc.encode(prefix), 345);
  return h;
}

function entry(name: string, body: string | Uint8Array, type = "0", prefix = ""): Uint8Array[] {
  const data = typeof body === "string" ? enc.encode(body) : body;
  const pad = new Uint8Array(Math.ceil(data.length / 512) * 512 - data.length);
  return [header(name, data.length, type, prefix), data, pad];
}

function pax(path: string): Uint8Array[] {
  const body = ` path=${path}\n`;
  const len = enc.encode(body).length;
  let total = len + String(len).length;
  if (String(total).length !== String(len).length) total++;
  return entry("PaxHeader", `${total}${body}`, "x");
}

function stream(parts: Uint8Array[], chunk = 700): ReadableStream<Uint8Array> {
  const all = new Uint8Array(parts.reduce((n, p) => n + p.length, 0) + 1024);
  let o = 0;
  for (const p of parts) {
    all.set(p, o);
    o += p.length;
  }
  let i = 0;
  return new ReadableStream({
    pull(c) {
      if (i >= all.length) return c.close();
      c.enqueue(all.subarray(i, i + chunk));
      i += chunk;
    },
  });
}

const EXT = [".java", ".ts", ".tsx"];

describe("PR path parsing", () => {
  it("accepts pull paths with or without sub-path", () => {
    expect(parsePrPath("/o/r/pull/12")).toEqual({ owner: "o", repo: "r", number: 12 });
    expect(parsePrPath("/o/my.repo-x/pull/3/files")).toEqual({ owner: "o", repo: "my.repo-x", number: 3 });
  });
  it("rejects other pages", () => {
    expect(parsePrPath("/o/r/pulls")).toBeNull();
    expect(parsePrPath("/o/r/issues/4")).toBeNull();
    expect(parsePrPath("/o/r/pull/x")).toBeNull();
    expect(parsePrPath("/")).toBeNull();
  });
  it("parses the viewer param and the shorthand", () => {
    expect(parsePrParam("o/r/7")).toEqual({ owner: "o", repo: "r", number: 7 });
    expect(parsePrParam("o/r")).toBeNull();
    expect(parsePrParam(null)).toBeNull();
    expect(parsePrShorthand("mybatis/jpetstore-6#1215")).toEqual({ owner: "mybatis", repo: "jpetstore-6", number: 1215 });
    expect(parsePrShorthand("nope")).toBeNull();
  });
});

describe("readTar", () => {
  it("keeps wanted regular files, strips the top directory, skips the rest", async () => {
    const parts = [
      ...entry("top/", "", "5"),
      ...entry("top/src/A.java", "class A {}"),
      ...entry("top/README.md", "hello"),
      ...entry("top/web/b.tsx", "x".repeat(1000)),
      ...entry("top/link.ts", "", "2"),
      ...entry("top/c.ts", "// é"),
    ];
    const files = await readTar(stream(parts), { extensions: EXT });
    expect(files.map((f) => f.path)).toEqual(["src/A.java", "web/b.tsx", "c.ts"]);
    expect(files[0].content).toBe("class A {}");
    expect(files[1].content).toHaveLength(1000);
    expect(files[2].content).toBe("// é");
  });

  it("joins the ustar prefix", async () => {
    const files = await readTar(stream(entry("B.java", "b", "0", "top/deep/dir")), { extensions: EXT });
    expect(files).toEqual([{ path: "deep/dir/B.java", content: "b" }]);
  });

  it("uses pax path and skips global headers", async () => {
    const long = "top/" + "d/".repeat(80) + "P.java";
    const parts = [...entry("pax_global_header", "52 comment=abc\n", "g"), ...pax(long), ...entry("trunc", "p")];
    const files = await readTar(stream(parts), { extensions: EXT });
    expect(files).toEqual([{ path: long.slice(4), content: "p" }]);
  });

  it("uses GNU long names", async () => {
    const long = "top/" + "n/".repeat(90) + "L.ts";
    const parts = [...entry("././@LongLink", long + "\0", "L"), ...entry("short", "l")];
    const files = await readTar(stream(parts), { extensions: EXT });
    expect(files).toEqual([{ path: long.slice(4), content: "l" }]);
  });

  it("refuses too much source text", async () => {
    const parts = [...entry("t/a.java", "x".repeat(600)), ...entry("t/b.java", "y".repeat(600))];
    await expect(readTar(stream(parts), { extensions: EXT, maxBytes: 1000 })).rejects.toMatchObject({
      kind: "too-large",
    });
  });
});

const res = (status: number, headers: Record<string, string> = {}) => ({
  status,
  headers: { get: (n: string) => headers[n.toLowerCase()] ?? null },
});

describe("githubError", () => {
  it("maps rate limit, not found, diff too large, auth", () => {
    expect(githubError(res(403, { "x-ratelimit-remaining": "0" }), "pr")).toMatchObject({ kind: "rate-limit" });
    expect(githubError(res(401, { "x-ratelimit-remaining": "0" }), "pr").message).toMatch(/token/);
    expect(githubError(res(404), "pr")).toMatchObject({ kind: "not-found" });
    expect(githubError(res(406), "diff")).toMatchObject({ kind: "diff-too-large" });
    expect(githubError(res(403, { "x-ratelimit-remaining": "12" }), "pr")).toMatchObject({ kind: "auth" });
    expect(githubError(res(500), "sources")).toMatchObject({ kind: "http" });
  });
});

describe("fetchPr", () => {
  const ref = { owner: "o", repo: "r", number: 1 };
  const json = (o: unknown) => new Response(JSON.stringify(o), { status: 200 });

  it("sends the token to api.github.com only and maps a 404", async () => {
    const calls: { url: string; headers: Record<string, string> }[] = [];
    const f = (async (url: string, init: RequestInit) => {
      calls.push({ url, headers: init.headers as Record<string, string> });
      return new Response("", { status: 404 });
    }) as unknown as typeof fetch;
    await expect(fetchPr(ref, { token: "t0k", fetch: f })).rejects.toBeInstanceOf(PrError);
    expect(calls[0].url).toBe("https://api.github.com/repos/o/r/pulls/1");
    expect(calls[0].headers.Authorization).toBe("Bearer t0k");
    expect(calls[0].headers["X-GitHub-Api-Version"]).toBe("2022-11-28");
  });

  it("maps a 406 on the diff", async () => {
    const f = (async (_url: string, init: RequestInit) => {
      const accept = (init.headers as Record<string, string>).Accept;
      return accept.includes("diff") ? new Response("", { status: 406 }) : json({ base: { sha: "b" }, head: { sha: "h" } });
    }) as unknown as typeof fetch;
    await expect(fetchPr(ref, { fetch: f })).rejects.toMatchObject({ kind: "diff-too-large" });
  });

  it("omits Authorization without a token", async () => {
    let seen: Record<string, string> = {};
    const f = (async (_url: string, init: RequestInit) => {
      seen = init.headers as Record<string, string>;
      return new Response("", { status: 404 });
    }) as unknown as typeof fetch;
    await fetchPr(ref, { fetch: f }).catch(() => {});
    expect("Authorization" in seen).toBe(false);
  });
});
