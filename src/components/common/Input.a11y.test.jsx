/**
 * Input.a11y.test.jsx — Axe accessibility regression tests for the shared
 * Input component (src/components/common/Input.jsx).
 *
 * Dependencies: vitest-axe (added by Agent 2 in this wave).
 * Environment:  jsdom (vitest.config.js).
 *
 * NOTE: color-contrast rule is disabled because jsdom cannot compute computed
 * CSS values. All other serious and critical axe violations are asserted.
 */

import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'vitest-axe';

import Input from './Input';

expect.extend(toHaveNoViolations);

/* ── Constants ──────────────────────────────────────────────────────────────── */

const AXE_OPTIONS = {
  rules: {
    'color-contrast': { enabled: false },
  },
};

/* ── Tests ──────────────────────────────────────────────────────────────────── */

describe('Input component — axe accessibility', () => {
  it('labelled text input has no violations', async () => {
    const { container } = render(<Input label="Email address" />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('required input with aria-required has no violations', async () => {
    const { container } = render(<Input label="Username" isRequired />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('input with error state has valid aria-invalid + role=alert', async () => {
    const { container } = render(
      <Input label="Email" error="Please enter a valid email address" />
    );
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('input with helper text has no violations', async () => {
    const { container } = render(
      <Input label="Password" type="password" helperText="Minimum 8 characters" />
    );
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('password input with toggle button has no violations', async () => {
    const { container } = render(<Input label="Password" type="password" />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('disabled input has no violations', async () => {
    const { container } = render(<Input label="Read-only field" isDisabled value="some value" readOnly />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('floating-label variant has no violations', async () => {
    const { container } = render(
      <Input label="Search" floatingLabel placeholder="Type to search…" />
    );
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });
});
