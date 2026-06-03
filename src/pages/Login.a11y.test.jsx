/**
 * Login.a11y.test.jsx — Axe accessibility regression tests for the real Login
 * page (src/pages/Login.jsx).
 *
 * The real page is rendered inside a MemoryRouter (it uses <Link>) with useAuth
 * and the OAuth-URL helper mocked so no network/auth side effects run. axe scans
 * the full form (email/password inputs + OAuth buttons + sign-in submit).
 *
 * Every axe() call is awaited and tests are non-concurrent (see
 * vitest.a11y.config.js) so the shared jsdom axe instance is never re-entered.
 *
 * NOTE: color-contrast is disabled because jsdom cannot compute CSS values.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { axe, toHaveNoViolations } from 'vitest-axe';

import Login from './Login';

expect.extend(toHaveNoViolations);

vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ login: vi.fn().mockResolvedValue({}) }),
}));

vi.mock('../api/client', () => ({
  getOAuthUrl: (provider) => `https://auth.example.test/${provider}`,
}));

const AXE_OPTIONS = {
  rules: {
    'color-contrast': { enabled: false },
  },
};

function renderLogin() {
  const { container } = render(
    <MemoryRouter>
      <Login />
    </MemoryRouter>
  );
  return container;
}

describe('Login page — axe accessibility', () => {
  it('renders the real login form with no serious/critical violations', async () => {
    const container = renderLogin();
    // Confirm we rendered the real page, not a stub.
    expect(screen.getByRole('heading', { name: /welcome back/i })).toBeInTheDocument();
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('email and password fields have accessible names/labels', async () => {
    const container = renderLogin();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['label', 'label-title-only', 'aria-input-field-name'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('all OAuth provider buttons have accessible names', async () => {
    const container = renderLogin();
    expect(screen.getByRole('button', { name: /continue with google/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /continue with github/i })).toBeInTheDocument();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['button-name'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('has exactly one main landmark / no duplicate-id violations', async () => {
    const container = renderLogin();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['duplicate-id', 'duplicate-id-active', 'landmark-no-duplicate-main'] },
    });
    expect(results).toHaveNoViolations();
  });
});
