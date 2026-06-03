/**
 * Marketplace.a11y.test.jsx — Axe accessibility regression tests for the real
 * Marketplace page (src/pages/Marketplace.jsx).
 *
 * The real page renders inside <Layout> (Navbar + Footer) and a list of
 * GameCards. We render the REAL page in a MemoryRouter and mock the data hooks
 * (useGames) plus the Navbar's auth/wallet/presence hooks so no network effects
 * run and the games grid renders deterministically.
 *
 * Every axe() call is awaited and tests are non-concurrent (vitest.a11y.config.js)
 * so the shared jsdom axe instance is never re-entered while a run is in flight.
 *
 * NOTE: color-contrast is disabled because jsdom cannot compute CSS values.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { axe, toHaveNoViolations } from 'vitest-axe';

import { mockGames } from '../data/mockGames';
import Marketplace from './Marketplace';

expect.extend(toHaveNoViolations);

// Deterministic games list for the grid.
vi.mock('../hooks/useGames', () => ({
  useGames: () => ({ games: mockGames, loading: false, error: null }),
}));

// Navbar (inside Layout) depends on these — stub them so no api effects run.
vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({ user: null, logout: vi.fn() }),
}));
vi.mock('../hooks/useWallet', () => ({
  useWallet: () => ({ balance: 0 }),
}));
vi.mock('../hooks/usePresence', () => ({
  usePresence: () => ({ getPresence: () => ({ status: 'offline' }) }),
}));

const AXE_OPTIONS = {
  rules: {
    'color-contrast': { enabled: false },
  },
};

function renderMarketplace() {
  const { container } = render(
    <MemoryRouter>
      <Marketplace />
    </MemoryRouter>
  );
  return container;
}

describe('Marketplace page — axe accessibility', () => {
  beforeEach(() => {
    // Suppress the auto-starting onboarding tour for a stable render.
    localStorage.setItem('magnetite_marketplace_tour_done', 'true');
  });

  it('renders the real games grid with no serious/critical violations', async () => {
    const container = renderMarketplace();
    // Confirm the real page (and at least one real game) rendered.
    expect(screen.getByText(mockGames[0].title)).toBeInTheDocument();
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('search and filter controls have accessible names', async () => {
    const container = renderMarketplace();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: {
        type: 'rule',
        values: ['label', 'select-name', 'button-name', 'aria-input-field-name'],
      },
    });
    expect(results).toHaveNoViolations();
  });

  it('game card links and images have accessible names / alt text', async () => {
    const container = renderMarketplace();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['link-name', 'image-alt', 'role-img-alt'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('landmarks and headings are valid (no duplicate ids)', async () => {
    const container = renderMarketplace();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: {
        type: 'rule',
        values: ['duplicate-id', 'duplicate-id-active', 'landmark-no-duplicate-main', 'heading-order'],
      },
    });
    expect(results).toHaveNoViolations();
  });
});
