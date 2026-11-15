// Chrome extension build (pages + static files). The content script is a separate
// build (vite.content.config.ts) because it must be a single self-contained IIFE.
//
// The Vite root is `extension/` (so viewer.html / options.html land at the top of
// dist-extension/); Tailwind source detection starts at the root, hence the explicit
// `@source` in src/extension/extension.css.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));
const wasmPkg = here("../../crates/wasm/pkg");

function requireWasmPkg(): Plugin {
  return {
    name: "structure-require-wasm-pkg",
    buildStart() {
      if (!existsSync(`${wasmPkg}/structure_wasm.js`) || !existsSync(`${wasmPkg}/structure_wasm_bg.wasm`)) {
        this.error(`crates/wasm/pkg is missing: run scripts/build-wasm.sh first (needs wasm-pack).`);
      }
    },
  };
}

export default defineConfig({
  root: here("./extension"),
  publicDir: here("./extension/public"),
  plugins: [requireWasmPkg(), tailwindcss(), svelte({ configFile: here("./svelte.config.js") })],
  resolve: { alias: { $lib: here("./src/lib"), "@wasm": wasmPkg } },
  clearScreen: false,
  build: {
    outDir: here("./dist-extension"),
    emptyOutDir: true,
    rollupOptions: {
      input: { viewer: here("./extension/viewer.html"), options: here("./extension/options.html") },
    },
  },
});
