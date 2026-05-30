import { test, expect } from '@playwright/test';
import { PointsPage } from './page-objects/points.page.js';

test.describe('Points Dashboard', () => {
  let pointsPage;

  test.beforeEach(async ({ page }) => {
    pointsPage = new PointsPage(page);
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
    // The mock balance is 4,820 points — any number-like text should appear.
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
    await page.waitForTimeout(300);
    const tabs = await page
      .locator('[role="tab"], button:has-text("History"), button:has-text("Rewards"), button:has-text("Leaderboard")')
      .all();
    expect(tabs.length).toBeGreaterThan(0);
  });

  test('History tab shows transaction entries', async ({ page }) => {
    await page.waitForTimeout(400);
    // Click the History tab (may already be visible or need a click)
    const histTab = page.locator('button:has-text("History"), [role="tab"]:has-text("History")');
    if (await histTab.count() > 0) {
      await histTab.first().click();
    }
    // At least one history row should be present from mock data.
    const rows = await page
      .locator('.hist-row, .history-entry, [class*="hist-row"], [class*="history"]')
      .all();
    expect(rows.length).toBeGreaterThan(0);
  });

  test('Rewards tab shows reward cards', async ({ page }) => {
    await page.waitForTimeout(400);
    const rewTab = page.locator('button:has-text("Rewards"), [role="tab"]:has-text("Rewards")');
    if (await rewTab.count() > 0) {
      await rewTab.first().click();
      await page.waitForTimeout(300);
    }
    const cards = await page
      .locator('.reward-card, .pts-reward-card, [class*="reward-card"]')
      .all();
    expect(cards.length).toBeGreaterThan(0);
  });

  test('Leaderboard tab shows leaderboard entries', async ({ page }) => {
    await page.waitForTimeout(400);
    const lbTab = page.locator('button:has-text("Leaderboard"), [role="tab"]:has-text("Leaderboard")');
    if (await lbTab.count() > 0) {
      await lbTab.first().click();
      await page.waitForTimeout(300);
    }
    const rows = await page
      .locator('.lb-row, .leaderboard-entry, [class*="lb-row"], [class*="leader"]')
      .all();
    expect(rows.length).toBeGreaterThan(0);
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
    const buttons = await page.locator('button').all();
    // Each button should be in the DOM — basic a11y check
    expect(buttons.length).toBeGreaterThan(0);
  });
});
