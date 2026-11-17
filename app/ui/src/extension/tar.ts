// Streaming tar reader: ustar name + prefix, pax (`x` path/size, `g` skipped),
// GNU long names (`L`), regular files only. Plain TS (no enums) so Node can import it.

import { PrError } from "./github.ts";

/** Bytes of kept source text above which the repository is refused. */
export const MAX_SOURCE_BYTES = 60 * 1024 * 1024;

export interface TarOptions {
  /** Extensions to keep, with the dot, e.g. `.java`. */
  extensions: readonly string[];
  maxBytes?: number;
}

export interface TarFile {
  /** Path without the archive's top-level directory. */
  path: string;
  content: string;
}

class ByteReader {
  private reader: ReadableStreamDefaultReader<Uint8Array>;
  private cur: Uint8Array = new Uint8Array(0);
  private pos = 0;
  constructor(stream: ReadableStream<Uint8Array>) {
    this.reader = stream.getReader();
  }
  private async fill(): Promise<boolean> {
    while (this.pos >= this.cur.length) {
      const { done, value } = await this.reader.read();
      if (done) return false;
      this.cur = value;
      this.pos = 0;
    }
    return true;
  }
  /** Up to `n` bytes (fewer only at the end of the stream). */
  async read(n: number): Promise<Uint8Array> {
    const out = new Uint8Array(n);
    let got = 0;
    while (got < n && (await this.fill())) {
      const k = Math.min(n - got, this.cur.length - this.pos);
      out.set(this.cur.subarray(this.pos, this.pos + k), got);
      this.pos += k;
      got += k;
    }
    return got < n ? out.subarray(0, got) : out;
  }
  async skip(n: number): Promise<void> {
    let left = n;
    while (left > 0 && (await this.fill())) {
      const k = Math.min(left, this.cur.length - this.pos);
      this.pos += k;
      left -= k;
    }
  }
}

const utf8 = new TextDecoder("utf-8", { fatal: false });

function cstr(b: Uint8Array, start: number, len: number): string {
  let end = start;
  while (end < start + len && b[end] !== 0) end++;
  return utf8.decode(b.subarray(start, end));
}

function parseSize(b: Uint8Array): number {
  if (b[124] & 0x80) {
    // base-256 (GNU) for sizes that do not fit in octal
    let v = b[124] & 0x7f;
    for (let i = 125; i < 136; i++) v = v * 256 + b[i];
    return v;
  }
  const s = cstr(b, 124, 12).trim();
  return s ? parseInt(s, 8) : 0;
}

/** Pax records `"<len> <key>=<value>\n"`; `len` counts bytes of the whole record. */
function parsePax(data: Uint8Array): Map<string, string> {
  const out = new Map<string, string>();
  let i = 0;
  while (i < data.length) {
    let sp = i;
    while (sp < data.length && data[sp] !== 0x20) sp++;
    const len = parseInt(utf8.decode(data.subarray(i, sp)), 10);
    if (!Number.isFinite(len) || len <= 0 || i + len > data.length) break;
    const rec = utf8.decode(data.subarray(sp + 1, i + len - 1));
    const eq = rec.indexOf("=");
    if (eq > 0) out.set(rec.slice(0, eq), rec.slice(eq + 1));
    i += len;
  }
  return out;
}

function isZero(b: Uint8Array): boolean {
  for (let i = 0; i < b.length; i++) if (b[i] !== 0) return false;
  return true;
}

const padded = (n: number) => Math.ceil(n / 512) * 512;

export async function readTar(stream: ReadableStream<Uint8Array>, opts: TarOptions): Promise<TarFile[]> {
  const max = opts.maxBytes ?? MAX_SOURCE_BYTES;
  const r = new ByteReader(stream);
  const files: TarFile[] = [];
  let kept = 0;
  let longName: string | null = null;
  let pax: Map<string, string> | null = null;
  for (;;) {
    const h = await r.read(512);
    if (h.length < 512 || isZero(h)) break;
    const type = String.fromCharCode(h[156] || 0x30);
    let size = parseSize(h);
    if (type === "x" || type === "L" || type === "K" || type === "g") {
      const data = await r.read(size);
      await r.skip(padded(size) - size);
      if (type === "x") pax = parsePax(data);
      else if (type === "L") longName = cstr(data, 0, data.length);
      continue;
    }
    const paxSize = pax?.get("size");
    if (paxSize) size = parseInt(paxSize, 10);
    let name = cstr(h, 0, 100);
    if (cstr(h, 257, 5) === "ustar") {
      const prefix = cstr(h, 345, 155);
      if (prefix) name = `${prefix}/${name}`;
    }
    const full = pax?.get("path") ?? longName ?? name;
    pax = null;
    longName = null;
    const slash = full.indexOf("/");
    const rest = full.slice(slash + 1);
    const wanted = type === "0" && slash >= 0 && opts.extensions.some((e) => rest.endsWith(e));
    if (!wanted) {
      await r.skip(padded(size));
      continue;
    }
    kept += size;
    if (kept > max) {
      throw new PrError("too-large", "Repository too large: more than 60 MB of source text.");
    }
    const data = await r.read(size);
    await r.skip(padded(size) - size);
    files.push({ path: rest, content: utf8.decode(data) });
  }
  return files;
}
