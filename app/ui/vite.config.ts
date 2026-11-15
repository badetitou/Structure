import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: { alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) } },
  clearScreen: false,
  build: {
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        viewer: fileURLToPath(new URL("./viewer.html", import.meta.url)),
      },
    },
  },
  server: { port: 1420, strictPort: true },
});
