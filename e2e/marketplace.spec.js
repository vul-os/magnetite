import { test, expect } from '@playwright/test';
import { MarketplacePage } from './page-objects/marketplace.page.js';

test.describe('Marketplace', () => {
  let marketplacePage;

  test.beforeEach(async ({ page }) => {
    marketplacePage = new MarketplacePage(page);
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
    // Search uses .search-input class on the marketplace header
    await expect(page.locator('input[placeholder*="earch"], .search-input')).toBeVisible();
  });

  test('category filter pills present', async ({ page }) => {
    // Category pills nav: nav.category-pills with aria-label="Game categories"
    await expect(page.locator('nav[aria-label="Game categories"]')).toBeVisible();
  });
});
