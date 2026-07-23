import { test, expect } from '@playwright/test';
import { MarketplacePage } from './page-objects/marketplace.page.js';

// The catalogue comes from GET /api/v1/games (useGames → api.games.list). The
// app calls it cross-origin (VITE_API_URL, default http://localhost:8080), so a
// fulfilled response needs CORS headers or the browser rejects it. A small
// honest fixture lets the grid render without a live backend.
const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
  'Access-Control-Allow-Headers': '*',
};
const GAMES = [
  { id: 'g1', title: 'Voxel Frontier', developer: 'Redshift Labs', category: 'action',   is_free: true,  fee_per_session: 0,    players_online: 1240, rating: 4.6,  is_new: false },
  { id: 'g2', title: 'Nebula Drift',   developer: 'Orbital Studio', category: 'racing',   is_free: false, fee_per_session: 0.05, players_online: 830,  rating: 4.3,  is_new: false },
  { id: 'g3', title: 'Grid Tactics',   developer: 'Iron Meridian',  category: 'strategy', is_free: true,  fee_per_session: 0,    players_online: 88,   rating: null, is_new: true  },
];

test.describe('Marketplace', () => {
  let marketplacePage;

  test.beforeEach(async ({ page }) => {
    marketplacePage = new MarketplacePage(page);
    await page.route('**/api/v1/games', async (route) => {
      if (route.request().method() === 'OPTIONS') {
        await route.fulfill({ status: 204, headers: CORS });
        return;
      }
      await route.fulfill({
        status: 200,
        headers: { ...CORS, 'Content-Type': 'application/json' },
        body: JSON.stringify({ games: GAMES }),
      });
    });
    await marketplacePage.navigate('/marketplace');
  });

  // Marketplace h1 reads "Discover Rust Games" (mkt-heading)
  test('marketplace page loads with correct heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /discover rust games/i })).toBeVisible();
  });

  // Game cards use class .game-card (GameCard component)
  test('game cards display', async ({ page: _page }) => {
    await marketplacePage.waitForLoading();
    const cardCount = await marketplacePage.getGameCardCount();
    expect(cardCount).toBeGreaterThan(0);
  });

  test('game card elements visible', async ({ page }) => {
    await marketplacePage.waitForLoading();
    // Each .game-card should be present after data loads from the API
    await expect(page.locator('.game-card').first()).toBeVisible();
  });

  test('search input is present', async ({ page }) => {
    // .search-input also appears in the navbar/mobile chrome, so scope to the
    // marketplace header's own search box (.header-content is unique to it).
    await expect(page.locator('.header-content .search-input')).toBeVisible();
  });

  test('category filter pills present', async ({ page }) => {
    // Category pills nav: nav.category-pills with aria-label="Game categories"
    await expect(page.locator('nav[aria-label="Game categories"]')).toBeVisible();
  });
});
