// Ambient module declaration for the `vitest-axe-real` specifier.
//
// This name only exists as a Vite `resolve.alias` entry (defined in both
// vitest.config.js and vitest.a11y.config.js) pointing at the real
// `vitest-axe` package's resolved entry point — see vitest-axe-shim.ts's
// header comment for why the alias is spelled this way. TypeScript has no
// visibility into Vite aliases, so this declaration gives `tsc` the same
// module graph the bundler resolves at runtime.
declare module 'vitest-axe-real' {
  export * from 'vitest-axe';
}
