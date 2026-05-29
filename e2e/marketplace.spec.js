import { test, expect } from '@playwright/test';
import { MarketplacePage } from './page-objects/marketplace.page.js';

test.describe('Marketplace', () => {
  let marketplacePage;

  test.beforeEach(async ({ page }) => {
    marketplacePage = new MarketplacePage(page);
    await marketplacePage.navigate('/marketplace');
  });

  test('marketplace page loads', async ({ page }) => {
    await expect(page.locator('h1')).toBeVisible();
  });

  test('game cards display', async ({ page: _page }) => {
    await marketplacePage.waitForLoading();
    const cardCount = await marketplacePage.getGameCardCount();
    expect(cardCount).toBeGreaterThan(0);
  });
});
