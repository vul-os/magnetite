/**
 * GameDetail.a11y.test.jsx — Axe accessibility regression tests for the real
 * GameDetail page (src/pages/GameDetail.jsx).
 *
 * The real page reads :id from the route and fetches the game/leaderboard/reviews
 * from the API in an effect. We render the REAL page inside a routed MemoryRouter
 * with ../api/client mocked to resolve a complete game, wait for the fetched
 * content to appear, then run axe against the fully-loaded page.
 *
 * GameDetail also uses an IntersectionObserver for its sticky buy-bar — the test
 * setup (src/test/setup.js) provides a jsdom stub for it.
 *
 * Every axe() call is awaited and tests are non-concurrent (vitest.a11y.config.js)
 * so the shared jsdom axe instance is never re-entered while a run is in flight.
 *
 * NOTE: color-contrast is disabled because jsdom cannot compute CSS values.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { axe, toHaveNoViolations } from 'vitest-axe';

import GameDetail from './GameDetail';

expect.extend(toHaveNoViolations);

// Defined via vi.hoisted so it's available inside the hoisted vi.mock factory.
const { GAME, LEADERBOARD, REVIEWS } = vi.hoisted(() => ({
  GAME: {
    id: '42',
    title: 'Cosmic Raiders',
    developer: 'StarForge Studios',
    developerId: 'starforge',
    requiredTier: 'free',
    isFree: true,
    rating: 4.7,
    category: 'Action',
    content_rating: 'everyone',
    description:
      'An interstellar adventure compiled to WebAssembly. Battle across 12 star systems.',
    thumbnail: 'https://example.test/thumb.jpg',
    screenshots: ['https://example.test/ss1.jpg', 'https://example.test/ss2.jpg'],
    video: null,
    github: null,
    release_date: '2026-03-15',
    players_min: 1,
    players_max: 4,
    system_requirements: {},
    achievements: [],
  },
  LEADERBOARD: [
    { rank: 1, player: 'NebulaKing', score: 15420, avatar: 'https://example.test/p1.jpg' },
    { rank: 2, player: 'SpaceAce', score: 14850, avatar: 'https://example.test/p2.jpg' },
  ],
  REVIEWS: [
    { user: 'GameMaster42', rating: 5, comment: 'Best space shooter!', date: '2026-05-10', helpful: 24 },
  ],
}));

// Mock the API client so the page's load effect resolves deterministically.
vi.mock('../api/client', () => ({
  api: {
    games: {
      get: vi.fn().mockResolvedValue(GAME),
      leaderboard: vi.fn().mockResolvedValue(LEADERBOARD),
    },
    reviews: {
      list: vi.fn().mockResolvedValue(REVIEWS),
      helpful: vi.fn().mockResolvedValue(null),
      report: vi.fn().mockResolvedValue(null),
    },
  },
}));

// Layout's Navbar depends on these — stub so no auth/wallet effects run.
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

/** Render the real GameDetail at /game/42 and wait for fetched content. */
async function renderGameDetail() {
  const { container } = render(
    <MemoryRouter initialEntries={['/game/42']}>
      <Routes>
        <Route path="/game/:id" element={<GameDetail />} />
      </Routes>
    </MemoryRouter>
  );
  // Wait until the async load resolves and the real game title renders.
  await screen.findByRole('heading', { name: /cosmic raiders/i });
  return container;
}

describe('GameDetail page — axe accessibility', () => {
  it('renders the loaded game page with no serious/critical violations', async () => {
    const container = await renderGameDetail();
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('tab controls have accessible names and valid roles', async () => {
    const container = await renderGameDetail();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: {
        type: 'rule',
        values: ['button-name', 'aria-allowed-attr', 'aria-roles', 'aria-required-attr'],
      },
    });
    expect(results).toHaveNoViolations();
  });

  it('images (screenshots/gallery/avatars) have alt text', async () => {
    const container = await renderGameDetail();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['image-alt', 'role-img-alt'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('links have accessible names and ids are unique', async () => {
    const container = await renderGameDetail();
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['link-name', 'duplicate-id', 'duplicate-id-active'] },
    });
    expect(results).toHaveNoViolations();
  });
});
