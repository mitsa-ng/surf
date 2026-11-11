import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    target: "safari15",
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        start: fileURLToPath(new URL("./start.html", import.meta.url)),
      },
    },
  },
  // vitest runs in node; svelte must resolve its browser build or mount()
  // throws lifecycle_function_unavailable.
  resolve: mode === "test" ? { conditions: ["browser"] } : {},
  test: {
    environment: "jsdom",
  },
}));
