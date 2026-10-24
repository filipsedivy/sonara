import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// The Tauri CLI serves the dev build on a fixed port (`devUrl` in src-tauri/tauri.conf.json) and
// bundles `dist/` for release. The page tests serve `dist/` with `vite preview`.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  preview: { port: 4173, strictPort: true },
  build: { outDir: "dist", sourcemap: false },
});
