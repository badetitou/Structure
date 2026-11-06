/** Display name of a class-level symbol id: `pkg.Outer.Inner` -> `Inner`,
 * `src/a/b.ts#Widget` -> `Widget`. The matrix no longer lists symbols, so
 * impact lists derive names from ids. */
export function symbolName(id: string): string {
  const hash = id.lastIndexOf("#");
  if (hash >= 0) return id.slice(hash + 1) || id;
  const dot = id.lastIndexOf(".");
  return dot >= 0 && dot < id.length - 1 ? id.slice(dot + 1) : id;
}
