// Content script of the extension: one self-contained IIFE file, no runtime imports.
// Runs after vite.extension.config.ts (which empties dist-extension/).
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  publicDir: false,
  clearScreen: false,
  build: {
    outDir: here("./dist-extension"),
    emptyOutDir: false,
    lib: {
      entry: here("./src/extension/content.ts"),
      formats: ["iife"],
      name: "StructureContent",
      fileName: () => "content.js",
    },
  },
});
