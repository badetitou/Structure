// Types of the wasm-pack output (crates/wasm/pkg), reached through the `@wasm` alias
// so that `npm run check` does not need the pkg to be built.
declare module "@wasm/structure_wasm.js" {
  export class PrAnalysis {
    constructor(files: string, diff: string, project: string, base: string, head: string);
    impact(): string;
    cell_symbols(cells: string, offset: number, limit: number): string;
  }
  export default function init(opts: { module_or_path: string | URL }): Promise<unknown>;
}
