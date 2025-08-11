import { defineConfig } from "vite";
import elmPlugin from "vite-plugin-elm";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  // Configuration for WASM-based CAD application
  clearScreen: false,
  plugins: [elmPlugin({ debug: false })],
  server: {
    port: 1420,
    strictPort: false, // Allow fallback to other ports
    fs: {
      // Allow serving files from the pkg directory (WASM build output)
      allow: ['..', './pkg']
    },
    headers: {
      // Required headers for WASM and SharedArrayBuffer support
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp'
    }
  },
  build: {
    target: 'es2020',
    rollupOptions: {
      external: [],
    },
  },
  optimizeDeps: {
    exclude: ['manifold-3d'], // Don't pre-bundle manifold-3d as it contains WASM
  },
  worker: {
    format: 'es',
  },
}));
