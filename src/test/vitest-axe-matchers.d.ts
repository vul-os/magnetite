// Type-only augmentation teaching `expect(...).toHaveNoViolations()` to tsc.
//
// Mirrors vitest-axe/extend-expect.d.ts's module-augmentation declaration
// exactly, but as a standalone type import rather than importing the real
// 'vitest-axe/extend-expect' module. That module's *runtime* JS calls
// `expect.extend()` with the package's own (unfiltered) toHaveNoViolations —
// which would double-register the matcher and override the shim's
// serious/critical-only filtering that vitest-axe-shim.ts installs (see its
// header comment). `import type` is erased at compile time, so this file has
// zero runtime effect; each *.a11y.test.tsx still registers the shim's
// filtered matcher itself via `expect.extend(toHaveNoViolations)`.
import type { AxeMatchers } from 'vitest-axe/matchers';

declare module 'vitest' {
  interface Assertion extends AxeMatchers {}
}
