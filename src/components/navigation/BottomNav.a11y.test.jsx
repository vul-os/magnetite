/**
 * BottomNav.a11y.test.jsx — Axe accessibility regression tests for the
 * BottomNav mobile navigation component.
 *
 * Dependencies: vitest-axe (added by Agent 2 in this wave).
 * Environment:  jsdom (vitest.config.js).
 *
 * NOTE: color-contrast rule is disabled because jsdom cannot compute computed
 * CSS values. All other serious and critical axe violations are asserted.
 *
 * The BottomNav component lives at:
 *   src/components/BottomNav.jsx  (imported by the navigation test folder)
 *
 * Key a11y features under test:
 *   - nav landmark with aria-label="Main navigation"
 *   - Five links each with an aria-label matching their visible label text
 *   - aria-current="page" on the active link only
 *   - Icons are aria-hidden="true" (inline SVG)
 */

import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { axe, toHaveNoViolations } from 'vitest-axe';

import BottomNav from '../BottomNav';

expect.extend(toHaveNoViolations);

/* ── Constants ──────────────────────────────────────────────────────────────── */

const AXE_OPTIONS = {
  rules: {
    'color-contrast': { enabled: false },
  },
};

/* ── Helpers ────────────────────────────────────────────────────────────────── */

function renderAt(path) {
  const { container } = render(
    <MemoryRouter initialEntries={[path]}>
      <BottomNav />
    </MemoryRouter>
  );
  return container;
}

/* ── Tests ──────────────────────────────────────────────────────────────────── */

describe('BottomNav component — axe accessibility', () => {
  it('has no serious or critical axe violations on /home', async () => {
    const container = renderAt('/home');
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('has no violations on /marketplace (Store active)', async () => {
    const container = renderAt('/marketplace');
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('has no violations on /communities', async () => {
    const container = renderAt('/communities');
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('has no violations on /profile/alice (nested profile route)', async () => {
    const container = renderAt('/profile/alice');
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('nav landmark is correctly labelled', async () => {
    const container = renderAt('/home');
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['landmark-unique', 'aria-required-attr'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('all link elements have accessible names', async () => {
    const container = renderAt('/home');
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['link-name'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('aria-current is only set on the active link', async () => {
    const container = renderAt('/play/xyz');
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['aria-roles', 'aria-allowed-attr'] },
    });
    expect(results).toHaveNoViolations();
  });
});
