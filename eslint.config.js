import path from 'node:path'
import { fileURLToPath } from 'node:url'
import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

const tsconfigRootDir = path.dirname(fileURLToPath(import.meta.url))

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
      ...tseslint.configs.recommendedTypeChecked,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2021 },
      parserOptions: { ecmaFeatures: { jsx: true }, projectService: true, tsconfigRootDir },
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
      // Of the first 90 no-misused-promises findings surfaced when type-aware
      // linting was turned on, 85 were "Promise-returning function provided
      // to attribute" — async event handlers (onClick/onSubmit/onChange/...)
      // passed straight to JSX. Every one sampled (~40+ checked by hand)
      // already catches its own errors into loading/error state; none was a
      // genuine unhandled-rejection risk. `checksVoidReturn.attributes: false`
      // is typescript-eslint's own documented option for exactly this
      // JSX-handler false-positive; it leaves the rule's other checks (bare
      // conditionals, arguments, IIFE callbacks — where the remaining 5
      // findings lived) fully active.
      '@typescript-eslint/no-misused-promises': ['error', { checksVoidReturn: { attributes: false } }],
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
      ...tseslint.configs.recommendedTypeChecked,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.node, ...globals.browser },
      parserOptions: { ecmaFeatures: { jsx: true }, projectService: true, tsconfigRootDir },
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
      ...tseslint.configs.recommendedTypeChecked,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2021 },
      parserOptions: { projectService: true, tsconfigRootDir },
    },
    rules: {
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    },
  },

  // TS/TSX test files: downgrade (not disable) the no-unsafe-* family.
  // Measured: 22 findings across 5 files (client.test.ts 9, GameStudio.test.tsx
  // 5, useGamepad.test.ts 3, useSearch.test.ts 3, useAuth.test.ts 2), each
  // hand-checked. Every one is either (a) JSON.parse of a fixture the same
  // test just wrote to localStorage/produced via encodeInputFrame, asserted
  // against immediately after, or (b) a vi.fn() mock's resolved value read
  // straight back out. None hid a production defect the way the same rules
  // did in src/** and magnetite-web-client/src/** (JSON.parse boundaries,
  // Array(n)-returns-any[], recharts' loosely-typed tickFormatter — all
  // fixed there, not downgraded). Kept as `warn` rather than off: still
  // signal, not a merge blocker for ephemeral test-fixture shapes.
  {
    files: ['**/*.{test,spec}.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-unsafe-assignment': 'warn',
      '@typescript-eslint/no-unsafe-member-access': 'warn',
      '@typescript-eslint/no-unsafe-return': 'warn',
      '@typescript-eslint/no-unsafe-argument': 'warn',
      '@typescript-eslint/no-unsafe-call': 'warn',
    },
  },
])
