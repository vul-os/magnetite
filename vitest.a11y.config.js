import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

// ─────────────────────────────────────────────────────────────────────────────
// Dedicated config for the page-level axe accessibility suite.
//
// axe-core keeps a single global "is running" guard per jsdom instance. When
// Vitest runs multiple *.a11y.test.jsx files in parallel (or runs individual
// tests concurrently) two axe() calls can overlap and throw
// "Axe is already running. Use await to wait for the previous run to finish".
//
// To make this robust we force fully serial execution:
//   - fileParallelism: false   → one test file at a time (no worker overlap)
//   - sequence.concurrent: false → tests within a file never run concurrently
//
// Run it with:  npm run test:a11y   (== vitest run --config vitest.a11y.config.js)
// ─────────────────────────────────────────────────────────────────────────────
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Same shim the main config uses so `import { axe, toHaveNoViolations }
      // from 'vitest-axe'` resolves to the serious/critical-filtering matcher.
      'vitest-axe': resolve(__dirname, 'src/test/vitest-axe-shim.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.js'],
    include: ['src/**/*.a11y.test.jsx'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**', '**/target/**'],
    // Serialize axe so two runs are never in flight at once.
    fileParallelism: false,
    sequence: {
      concurrent: false,
    },
    // Generous but bounded so a stuck axe run fails loudly instead of hanging.
    testTimeout: 20000,
  },
});
