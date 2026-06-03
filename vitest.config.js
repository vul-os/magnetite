import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Shim so test files can `import { axe, toHaveNoViolations } from 'vitest-axe'`
      // even though the package only exports toHaveNoViolations from vitest-axe/matchers.
      'vitest-axe': resolve(__dirname, 'src/test/vitest-axe-shim.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.js'],
    include: ['src/**/*.{test,spec}.{js,jsx}', 'magnetite-web-client/src/**/*.{test,spec}.{js,jsx}'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**', '**/target/**'],
  },
});
