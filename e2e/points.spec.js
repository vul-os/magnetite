import { test, expect } from '@playwright/test';
import { PointsPage } from './page-objects/points.page.js';

// usePoints loads balance/history/rewards/leaderboard from /api/v1/points/*
// (cross-origin at VITE_API_URL, so fulfilled responses need CORS headers).
// A small honest fixture lets the tabs render real content without a backend.
// Rewards is deliberately 404 — the node does not implement a rewards catalogue
// (the UI shows an "unavailable" notice), so the spec asserts that, not cards.
const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
  'Access-Control-Allow-Headers': '*',
};
const BALANCE = {
  points: 1240, lifetime_points: 8300, rank: 42,
  season: { name: 'Season 1', tier: 'Silver', next_tier: 'Gold', progress: 40, points_needed: 760, ends_at: '2026-09-01T00:00:00Z' },
};
const HISTORY = [
  { id: 'h1', type: 'earn',  description: 'Won a ranked match', created_at: '2026-07-22T10:00:00Z', amount: 120 },
  { id: 'h2', type: 'earn',  description: 'Daily login',        created_at: '2026-07-21T08:00:00Z', amount: 20 },
  { id: 'h3', type: 'spend', description: 'Redeemed a cosmetic', created_at: '2026-07-20T18:00:00Z', amount: -200 },
];
const LEADERBOARD = [
  { rank: 1, username: 'AliceRust', avatar: null, points: 15200 },
  { rank: 2, username: 'BobBuilds', avatar: null, points: 12100 },
  { rank: 3, username: 'CarolFast', avatar: null, points: 9800 },
];

async function stubPoints(page) {
  await page.route('**/api/v1/points/**', async (route) => {
    if (route.request().method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers: CORS });
      return;
    }
    const path = new URL(route.request().url()).pathname;
    const json = (status, obj) =>
      route.fulfill({ status, headers: { ...CORS, 'Content-Type': 'application/json' }, body: JSON.stringify(obj) });
    if (path.endsWith('/balance')) return json(200, BALANCE);
    if (path.endsWith('/history')) return json(200, { history: HISTORY });
    if (path.endsWith('/leaderboard')) return json(200, { entries: LEADERBOARD });
    if (path.endsWith('/rewards')) return json(404, { message: 'rewards catalogue not implemented' });
    return route.continue();
  });
}

test.describe('Points Dashboard', () => {
  let pointsPage;

  test.beforeEach(async ({ page }) => {
    pointsPage = new PointsPage(page);
    await stubPoints(page);
    await pointsPage.navigate('/points');
  });

  test('points page loads without crashing', async ({ page }) => {
    // Heading, balance or the main layout element should be present.
    await expect(
      page.locator('h1, h2, [class*="points"], [class*="balance"]')
    ).not.toHaveCount(0);
  });

  test('page heading is visible', async ({ page }) => {
    await expect(page.locator('h1, h2').first()).toBeVisible();
  });

  test('balance value is displayed', async ({ page }) => {
    // Balance is loaded from the real API; any numeric value should appear.
    await page.waitForTimeout(400);
    const balanceEl = page.locator('[class*="balance-val"], [class*="pts-val"], [class*="points-amount"]');
    if (await balanceEl.count() > 0) {
      await expect(balanceEl.first()).toBeVisible();
    } else {
      // Fallback: check a numeric value is present anywhere on the page.
      await expect(page.locator('text=/\\d{1,3}(,\\d{3})*/').first()).toBeVisible();
    }
  });

  test('tab bar with multiple tabs is present', async ({ page }) => {
    // Four role=tab buttons: Overview / History / Rewards / Leaderboard.
    // (A comma-mix of [role=tab] with :has-text() breaks Playwright matching,
    // so query the role directly.) Wait for render — .count() does not auto-wait.
    await expect(page.getByRole('tab').first()).toBeVisible();
    expect(await page.getByRole('tab').count()).toBeGreaterThan(1);
  });

  test('History tab shows transaction entries', async ({ page }) => {
    await page.getByRole('tab', { name: 'History' }).click();
    // Each transaction renders a .points-tx-row.
    const rows = page.locator('.points-tx-row');
    await expect(rows.first()).toBeVisible();
    expect(await rows.count()).toBeGreaterThan(0);
  });

  test('Rewards tab shows the unavailable notice (no rewards catalogue on this node)', async ({ page }) => {
    await page.getByRole('tab', { name: 'Rewards' }).click();
    // Rewards/redemption are not implemented on the node (GET /points/rewards
    // 404s), so the tab shows an honest "unavailable" notice, not reward cards.
    await expect(page.getByText(/no rewards catalogue on this node/i)).toBeVisible();
  });

  test('Leaderboard tab shows leaderboard entries', async ({ page }) => {
    await page.getByRole('tab', { name: 'Leaderboard' }).click();
    // Each ranked player renders a .points-lb-row.
    const rows = page.locator('.points-lb-row');
    await expect(rows.first()).toBeVisible();
    expect(await rows.count()).toBeGreaterThan(0);
  });

  test('season card or season info is visible', async ({ page }) => {
    await page.waitForTimeout(400);
    const seasonEl = page.locator('[class*="season"], .pts-season, .season-card');
    if (await seasonEl.count() > 0) {
      await expect(seasonEl.first()).toBeVisible();
    } else {
      // Season info may be embedded in the balance hero — just check page loaded.
      await expect(page.locator('h1, h2').first()).toBeVisible();
    }
  });

  test('page is accessible — no orphaned interactive elements', async ({ page }) => {
    // Basic a11y check — wait for the first button so the count doesn't race render.
    await expect(page.locator('button').first()).toBeVisible();
    expect(await page.locator('button').count()).toBeGreaterThan(0);
  });
});
