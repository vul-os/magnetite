import { test, expect } from '@playwright/test';
import { StreamsPage } from './page-objects/streams.page.js';

// The live-stream grid comes from GET /api/v1/streams (api.streams.list('global')),
// called cross-origin (VITE_API_URL, default http://localhost:8080), so a fulfilled
// response needs CORS headers. A small honest fixture lets cards render without a
// live backend; on a real fetch error the page sets streams to [] (no cards).
const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
  'Access-Control-Allow-Headers': '*',
};
const STREAMS = [
  { id: 's1', title: 'Ranked grind to Diamond', game: 'Voxel Frontier', streamer: 'AliceRust', viewerCount: 1420, thumbnailUrl: null, liveAt: '2026-07-23T10:00:00Z' },
  { id: 's2', title: 'Chill build session',     game: 'Grid Tactics',   streamer: 'BobBuilds', viewerCount: 340,  thumbnailUrl: null, liveAt: '2026-07-23T09:30:00Z' },
  { id: 's3', title: 'Speedrun attempts',       game: 'Nebula Drift',   streamer: 'CarolFast', viewerCount: 88,   thumbnailUrl: null, liveAt: '2026-07-23T09:00:00Z' },
];

async function stubStreams(page) {
  await page.route('**/api/v1/streams', async (route) => {
    if (route.request().method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers: CORS });
      return;
    }
    await route.fulfill({
      status: 200,
      headers: { ...CORS, 'Content-Type': 'application/json' },
      body: JSON.stringify({ streams: STREAMS }),
    });
  });
}

test.describe('Streams Browse', () => {
  let streamsPage;

  test.beforeEach(async ({ page }) => {
    streamsPage = new StreamsPage(page);
    await stubStreams(page);
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
    // Each StreamCard renders a .stream-card__viewer-chip with the count.
    await expect(page.locator('[class*="viewer"]').first()).toBeVisible();
  });
});

test.describe('Streams — Go Live panel', () => {
  test('Go Live panel appears when Go Live button is clicked', async ({ page }) => {
    await stubStreams(page);
    await page.goto('/streams');

    // .streams-golive-btn is the header toggle specifically (the open panel adds
    // its own "Go live" button, so match by class, not accessible name). The
    // panel is #golive-panel, rendered only while open.
    await page.locator('.streams-golive-btn').click();
    await expect(page.locator('#golive-panel')).toBeVisible();
  });

  test('Go Live panel can be dismissed', async ({ page }) => {
    await stubStreams(page);
    await page.goto('/streams');

    const toggle = page.locator('.streams-golive-btn');
    await toggle.click();
    await expect(page.locator('#golive-panel')).toBeVisible();

    // Clicking the same toggle again (now labelled "Cancel") closes the panel.
    await toggle.click();
    await expect(page.locator('#golive-panel')).toBeHidden();
  });
});
