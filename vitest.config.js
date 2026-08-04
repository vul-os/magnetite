import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { createRequire } from 'node:module';
import { resolve } from 'path';

const require = createRequire(import.meta.url);

export default defineConfig({
  plugins: [react()],
  resolve: {
    // Array form (not object) so `find` can be an anchored regex: a plain
    // string alias also matches sub-paths like `vitest-axe/matchers`, which
    // would send them to the shim too.
    alias: [
      // Shim so test files can `import { axe, toHaveNoViolations } from 'vitest-axe'`
      // even though the package only exports toHaveNoViolations from vitest-axe/matchers.
      {
        find: /^vitest-axe$/,
        replacement: resolve(__dirname, 'src/test/vitest-axe-shim.js'),
      },
      // The shim's non-recursive way back to the real package. Resolved through
      // Node's own resolver, so it honours the package `exports` map and does
      // not hardcode a path into node_modules.
      { find: /^vitest-axe-real$/, replacement: require.resolve('vitest-axe') },
    ],
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.js'],
    include: ['src/**/*.{test,spec}.{js,jsx,ts,tsx}', 'magnetite-web-client/src/**/*.{test,spec}.{js,jsx,ts,tsx}'],
    // The axe page/component a11y suite (*.a11y.test.jsx) is excluded here and
    // run separately via `npm run test:a11y` (vitest.a11y.config.js), which
    // forces serial execution so concurrent axe runs never collide in the
    // shared jsdom axe instance. Keeping them out keeps the main suite fast.
    exclude: ['e2e/**', 'node_modules/**', 'dist/**', '**/target/**', 'src/**/*.a11y.test.{jsx,tsx}'],
  },
});
