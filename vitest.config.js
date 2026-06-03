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
    // The axe page/component a11y suite (*.a11y.test.jsx) is excluded here and
    // run separately via `npm run test:a11y` (vitest.a11y.config.js), which
    // forces serial execution so concurrent axe runs never collide in the
    // shared jsdom axe instance. Keeping them out keeps the main suite fast.
    exclude: ['e2e/**', 'node_modules/**', 'dist/**', '**/target/**', 'src/**/*.a11y.test.jsx'],
  },
});
