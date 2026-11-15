// GitHub URL helpers and error mapping. Pure: no `chrome.*`, no DOM.

export interface PrRef {
  owner: string;
  repo: string;
  number: number;
}

const NAME = "[A-Za-z0-9_.-]+";

/** `/<owner>/<repo>/pull/<number>[/anything]` -> the pull request, else null. */
export function parsePrPath(pathname: string): PrRef | null {
  const m = new RegExp(`^/(${NAME})/(${NAME})/pull/(\\d+)(?:/.*)?$`).exec(pathname);
  if (!m) return null;
  return { owner: m[1], repo: m[2], number: Number(m[3]) };
}

/** `<owner>/<repo>/<number>` (the viewer's `pr` query value). */
export function parsePrParam(value: string | null): PrRef | null {
  const m = value ? new RegExp(`^(${NAME})/(${NAME})/(\\d+)$`).exec(value) : null;
  return m ? { owner: m[1], repo: m[2], number: Number(m[3]) } : null;
}

/** Inverse of `parsePrParam`. */
export function prParam(ref: PrRef): string {
  return `${ref.owner}/${ref.repo}/${ref.number}`;
}

/** `<owner>/<repo>#<number>` (command line form). */
export function parsePrShorthand(value: string): PrRef | null {
  const m = new RegExp(`^(${NAME})/(${NAME})#(\\d+)$`).exec(value);
  return m ? { owner: m[1], repo: m[2], number: Number(m[3]) } : null;
}

const API = "https://api.github.com";

export function prUrl(r: PrRef): string {
  return `${API}/repos/${r.owner}/${r.repo}/pulls/${r.number}`;
}

export function tarballUrl(r: PrRef, sha: string): string {
  return `${API}/repos/${r.owner}/${r.repo}/tarball/${sha}`;
}

/** Headers for api.github.com. The token goes here and nowhere else. */
export function apiHeaders(token: string | undefined, accept: string): Record<string, string> {
  const h: Record<string, string> = { Accept: accept, "X-GitHub-Api-Version": "2022-11-28" };
  if (token) h.Authorization = `Bearer ${token}`;
  return h;
}

export type PrErrorKind = "rate-limit" | "not-found" | "auth" | "diff-too-large" | "too-large" | "http" | "network";

export class PrError extends Error {
  readonly kind: PrErrorKind;
  constructor(kind: PrErrorKind, message: string) {
    super(message);
    this.name = "PrError";
    this.kind = kind;
  }
}

/** Maps a failed GitHub response to a readable error. `what` is "pr", "diff" or "sources". */
export function githubError(res: { status: number; headers: { get(n: string): string | null } }, what: string): PrError {
  const { status } = res;
  if ((status === 401 || status === 403 || status === 429) && res.headers.get("x-ratelimit-remaining") === "0") {
    return new PrError(
      "rate-limit",
      "GitHub API rate limit reached. Add a personal access token in the extension options to raise it.",
    );
  }
  if (status === 404) {
    return new PrError(
      "not-found",
      "Pull request not found, or the repository is private. For a private repository add a token in the extension options.",
    );
  }
  if (status === 406 && what === "diff") {
    return new PrError("diff-too-large", "The pull request diff is too large for GitHub to return it.");
  }
  if (status === 401 || status === 403) {
    return new PrError("auth", `GitHub refused access (HTTP ${status}). Check the token in the extension options.`);
  }
  return new PrError("http", `GitHub returned HTTP ${status} while fetching the ${what}.`);
}
