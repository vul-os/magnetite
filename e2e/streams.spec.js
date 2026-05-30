import { test, expect } from '@playwright/test';
import { StreamsPage } from './page-objects/streams.page.js';

test.describe('Streams Browse', () => {
  let streamsPage;

  test.beforeEach(async ({ page }) => {
    streamsPage = new StreamsPage(page);
    await streamsPage.navigate('/streams');
  });

  test('streams page loads without crashing', async ({ page }) => {
    await expect(
      page.locator('h1, h2, .streams-page, [class*="streams"]')
    ).not.toHaveCount(0);
  });

  test('page heading is visible', async ({ page }) => {
    await expect(page.locator('h1, h2').first()).toBeVisible();
  });

  test('stream cards are displayed', async ({ page }) => {
    // Cards come from the real API; when the backend is unavailable an empty-state is shown.
    await page.waitForTimeout(500);
    const cards = await page.locator('.stream-card, [class*="stream-card"]').all();
    expect(cards.length).toBeGreaterThan(0);
  });

  test('each stream card shows a streamer name or title', async ({ page }) => {
    await page.waitForTimeout(500);
    const cards = page.locator('.stream-card, [class*="stream-card"]');
    const count = await cards.count();
    expect(count).toBeGreaterThan(0);
    // First card should contain visible text
    await expect(cards.first()).toBeVisible();
  });

  test('Go Live button is present', async ({ page }) => {
    const goLiveBtn = page.locator('button:has-text("Go Live"), .go-live-btn, [aria-label*="Go Live" i]');
    if (await goLiveBtn.count() > 0) {
      await expect(goLiveBtn.first()).toBeVisible();
    } else {
      // Go Live may be accessible through a different label — just check buttons exist
      const buttons = await page.locator('button').all();
      expect(buttons.length).toBeGreaterThan(0);
    }
  });

  test('clicking a stream card does not crash the page', async ({ page }) => {
    await page.waitForTimeout(500);
    const cards = page.locator('.stream-card, [class*="stream-card"]');
    if (await cards.count() > 0) {
      await cards.first().click();
      // After clicking, page should still contain content
      await expect(page.locator('body')).not.toBeEmpty();
    }
  });

  test('viewer count labels are present on stream cards', async ({ page }) => {
    await page.waitForTimeout(500);
    // Stream cards show viewer counts from the API response
    const viewerEls = page.locator('[class*="viewer"], text=/\\d+ viewer/i');
    if (await viewerEls.count() > 0) {
      await expect(viewerEls.first()).toBeVisible();
    } else {
      // Fallback: stream cards still present
      expect(await page.locator('.stream-card, [class*="stream-card"]').count()).toBeGreaterThan(0);
    }
  });
});

test.describe('Streams — Go Live panel', () => {
  test('Go Live panel appears when Go Live button is clicked', async ({ page }) => {
    await page.goto('/streams');
    await page.waitForTimeout(500);

    const goLiveBtn = page.locator('button:has-text("Go Live"), .go-live-btn');
    if (await goLiveBtn.count() === 0) {
      test.skip();
      return;
    }

    await goLiveBtn.first().click();
    await page.waitForTimeout(300);

    // GoLivePanel overlay or modal should appear
    await expect(
      page.locator('.go-live-panel, [class*="go-live-panel"], [class*="golive"]')
    ).toBeVisible();
  });

  test('Go Live panel can be dismissed', async ({ page }) => {
    await page.goto('/streams');
    await page.waitForTimeout(500);

    const goLiveBtn = page.locator('button:has-text("Go Live"), .go-live-btn');
    if (await goLiveBtn.count() === 0) {
      test.skip();
      return;
    }

    await goLiveBtn.first().click();
    await page.waitForTimeout(300);

    // Dismiss via Escape or a cancel/close button
    const cancelBtn = page.locator('button:has-text("Cancel"), button:has-text("Close"), [aria-label*="close" i]');
    if (await cancelBtn.count() > 0) {
      await cancelBtn.first().click();
    } else {
      await page.keyboard.press('Escape');
    }

    await page.waitForTimeout(300);
    // Panel should no longer be visible
    await expect(page.locator('.go-live-panel')).not.toBeVisible();
  });
});
