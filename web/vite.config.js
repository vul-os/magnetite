import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // Dedicated dev port for Magnetite (5173 is commonly taken by other Vite
    // apps). strictPort makes Vite FAIL LOUDLY if this port is busy instead of
    // silently bumping to the next free port — the auto-bump is what breaks HMR
    // (server moves to :5174 but the HMR websocket still targets :5173), leaving
    // the browser stuck on a stale bundle.
    port: 5174,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          // Monaco editor — large; lazy-loaded only when CodeEditor mounts.
          // Keep ALL monaco internals in a single chunk for better caching.
          if (id.includes('node_modules/monaco-editor') ||
              id.includes('node_modules/@monaco-editor')) {
            return 'vendor-monaco'
          }

          // recharts + its deps (d3-*, victory-*, etc.) — heavy, dashboard-only
          if (id.includes('node_modules/recharts') ||
              id.includes('node_modules/d3-') ||
              id.includes('node_modules/victory-') ||
              id.includes('node_modules/eventemitter3') ||
              id.includes('node_modules/lodash')) {
            return 'vendor-recharts'
          }

          // react-router-dom + router internals
          if (id.includes('node_modules/react-router') ||
              id.includes('node_modules/@remix-run')) {
            return 'vendor-router'
          }

          // react + react-dom core runtime (smallest, shared by all routes)
          if (id.includes('node_modules/react-dom') ||
              id.includes('node_modules/react/')) {
            return 'vendor-react'
          }
        },
      },
    },
  },
})
