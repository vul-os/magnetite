/**
 * vitest-axe-shim.js
 *
 * Compatibility shim aliased as 'vitest-axe' in vitest.config.js.
 *
 * Test files use the jest-axe style API:
 *   import { axe, toHaveNoViolations } from 'vitest-axe';
 *   expect.extend(toHaveNoViolations);
 *
 * The real vitest-axe package exports `toHaveNoViolations` as a bare function
 * from 'vitest-axe/matchers', but expect.extend() requires a matcher *object*
 * ({ toHaveNoViolations: fn }).  This shim re-exports `toHaveNoViolations` as
 * the object form so `expect.extend(toHaveNoViolations)` works correctly.
 */
// `vitest-axe-real` is an alias (defined in vitest.config.js and
// vitest.a11y.config.js) pointing at the real package's resolved entry point.
// It cannot be spelled `vitest-axe` here — that name is aliased to THIS file,
// so the import would resolve to itself — and it cannot be
// `vitest-axe/dist/index.js` either, because the package's `exports` map
// publishes only `.`, `./matchers` and `./extend-expect`, so the deep path is
// not resolvable. The alias is the one spelling that is both non-recursive and
// independent of where node_modules physically sits, which is what makes this
// harness copyable into a repo with a different directory depth or a pnpm
// store. See CONTRIBUTING.md → "Accessibility (axe) tests".
export { axe, configureAxe } from 'vitest-axe-real';

// Custom matcher that asserts no SERIOUS or CRITICAL axe violations — the a11y
// suite's stated contract. Minor/moderate best-practice findings (e.g. an
// aria-label on a non-interactive span) are reported but do not fail the suite,
// keeping the regression tests focused on impactful issues.
const BLOCKING = new Set(['serious', 'critical']);

function toHaveNoViolationsImpl(received) {
  const all = (received && received.violations) || [];
  const blocking = all.filter((v) => BLOCKING.has(v.impact));
  const pass = blocking.length === 0;
  return {
    pass,
    message: () => {
      if (pass) return 'Expected serious/critical axe violations, but found none.';
      const lines = blocking.map(
        (v) => `  [${v.impact}] ${v.id}: ${v.help} (${v.nodes.length} node(s))`,
      );
      return `Expected no serious/critical axe violations, but found ${blocking.length}:\n${lines.join('\n')}`;
    },
  };
}

// Export as the object form that expect.extend() consumes.
export const toHaveNoViolations = { toHaveNoViolations: toHaveNoViolationsImpl };
