/// <reference types="vite/client" />

// Vite's own ImportMetaEnv falls back to `Record<string, any>` for every
// VITE_* key unless a project opts into strict typing (see Vite's
// import.meta.env docs). Without this file, every `import.meta.env.VITE_*`
// access resolved as `any`, which is what surfaced as a cluster of
// @typescript-eslint/no-unsafe-* findings once type-aware linting was
// turned on — this is the real fix, not a suppression.
interface ViteTypeOptions {
  strictImportMetaEnv: unknown
}

interface ImportMetaEnv {
  readonly VITE_API_URL?: string
  readonly VITE_USE_MOCKS?: string
  readonly VITE_USE_MOCK_WS?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
