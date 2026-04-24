import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Vite config for the Vision web app.
// Builds to dist/, which is then served by server.ts (Bun) and bundled by Tauri.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:7777",
        changeOrigin: false,
      },
      "/ws": {
        target: "ws://127.0.0.1:7777",
        ws: true,
      },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    target: "es2022",
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom"],
          d3: ["d3", "d3-sankey"],
          gl: ["sigma", "graphology", "deck.gl", "three"],
        },
      },
    },
  },
  // Allow `mneme export --view <name>` to inline the bundle into a single .html
  // by setting VITE_INLINE_EXPORT=1 at build time.
  define: {
    __INLINE_EXPORT__: JSON.stringify(process.env.VITE_INLINE_EXPORT === "1"),
  },
});
