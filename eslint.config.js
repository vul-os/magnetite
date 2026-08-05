import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  // 'site/assets/vendor' holds third-party minified bundles (marked, mermaid)
  // vendored for the CDN-free landing page — never our code to lint.
  globalIgnores(['dist', 'dist-ssr', 'node_modules', '**/target/**', 'coverage', 'public/sw.js', 'site/assets/vendor']),

  // Application source (browser runtime). Only Node-side tooling configs and
  // public/sw.js remain plain JS — src/ and e2e/ are fully TypeScript (see
  // the TS blocks below).
  {
    files: ['**/*.{js,jsx}'],
    extends: [
      js.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2021 },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      // Context/provider modules legitimately export a component + a hook.
      'react-refresh/only-export-components': 'warn',
      // Experimental react-hooks rules: keep as signal, not blockers.
      'react-hooks/set-state-in-effect': 'warn',
      'react-hooks/refs': 'warn',
      'react-hooks/immutability': 'warn',
      'react-hooks/purity': 'warn',
      'react-hooks/exhaustive-deps': 'warn',
      'no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    },
  },

  // Test files (Vitest globals + jsdom)
  {
    files: ['**/*.{test,spec}.{js,jsx}', 'src/test/**/*.{js,jsx}'],
    languageOptions: {
      globals: { ...globals.browser, ...globals.node, ...globals.vitest },
    },
  },

  // Playwright e2e + Node-side tooling/config
  {
    files: ['e2e/**/*.{js,jsx}', '*.config.js', 'scripts/**/*.{js,jsx,mjs,cjs}'],
    languageOptions: {
      globals: { ...globals.node, ...globals.browser },
    },
  },

  // The TypeScript-migrated app source (src/**). Mirrors the JS block above
  // but swaps the parser/no-unused-vars rule for TS-aware equivalents.
  {
    files: ['src/**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2021 },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      'react-refresh/only-export-components': 'warn',
      'react-hooks/set-state-in-effect': 'warn',
      'react-hooks/refs': 'warn',
      'react-hooks/immutability': 'warn',
      'react-hooks/purity': 'warn',
      'react-hooks/exhaustive-deps': 'warn',
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      // src/test/vitest-axe-matchers.d.ts augments a third-party interface via
      // `interface Assertion extends AxeMatchers {}` — TypeScript's module-
      // augmentation / declaration-merging syntax requires exactly this empty-
      // body-extends-one-supertype shape; there is no other way to spell it.
      // 'with-single-extends' is the option the rule ships specifically for
      // this pattern, so this configures the rule rather than disabling it.
      '@typescript-eslint/no-empty-object-type': ['error', { allowInterfaces: 'with-single-extends' }],
    },
  },

  // The Playwright suite (e2e/**), migrated to TypeScript. Mirrors the src
  // block's TS-aware extends/parser rather than being syntax-only. It runs
  // in Node (the Playwright test runner) but page objects also author
  // inline page.evaluate-style callbacks that execute in the page, so both
  // global sets are legitimate.
  {
    files: ['e2e/**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.node, ...globals.browser },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      // Playwright's fixture API is `async ({ page }, use) => { await use(x) }`.
      // React 19 also has a `use` hook, so rules-of-hooks sees the call and
      // demands the enclosing function be a component/hook. It is neither —
      // this is a test fixture, not React.
      'react-hooks/rules-of-hooks': 'off',
    },
  },

  // magnetite-web-client/src is the standalone TS networking client bundled
  // into this package (no package.json of its own — it rides the root
  // tsconfig's "include"). It has no React/JSX; it runs in the browser.
  {
    files: ['magnetite-web-client/src/**/*.ts'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommended,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2021 },
    },
    rules: {
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    },
  },
])
