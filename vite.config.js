import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
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
