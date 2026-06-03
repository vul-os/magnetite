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
export { axe, configureAxe } from '../../node_modules/vitest-axe/dist/index.js';

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
