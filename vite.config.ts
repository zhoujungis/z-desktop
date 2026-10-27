import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  // Tauri expects a fixed port; fail if that port is not available
  server: {
    port: 5173,
    strictPort: true,
  },
  // Env variables starting with VITE_ are exposed to the frontend
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome110",
    minify: "esbuild",
    sourcemap: false,
  },
  clearScreen: false,
});
