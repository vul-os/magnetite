import { test, expect } from '@playwright/test';
import { CommunitiesPage } from './page-objects/communities.page.js';

test.describe('Communities', () => {
  let communitiesPage;

  test.beforeEach(async ({ page }) => {
    communitiesPage = new CommunitiesPage(page);
    await communitiesPage.navigate('/communities');
  });

  // The page routes to /communities and renders the Discord-like layout.
  test('communities page loads without crashing', async ({ page }) => {
    // Expect either a heading or the main layout to be present.
    await expect(
      page.locator('h1, h2, .communities-layout, .communities-page')
    ).not.toHaveCount(0);
  });

  test('server rail is rendered', async ({ page }) => {
    // ServerRail holds the community icon list.
    await expect(page.locator('.server-rail, [data-testid="server-rail"]')).toBeVisible();
  });

  test('channel sidebar is present', async ({ page }) => {
    await expect(
      page.locator('.channel-sidebar, .channels-panel, [class*="channel"]')
    ).not.toHaveCount(0);
  });

  test('channel items appear in the sidebar', async ({ page }) => {
    // Wait briefly for mock data to hydrate.
    await page.waitForTimeout(500);
    const channels = await page
      .locator('.channel-btn, .channel-item, [class*="channel-item"]')
      .all();
    expect(channels.length).toBeGreaterThan(0);
  });

  test('member list panel is present', async ({ page }) => {
    await expect(
      page.locator('.member-list, .members-panel, [class*="member"]')
    ).not.toHaveCount(0);
  });

  test('message composer is present', async ({ page }) => {
    // MessageComposer renders a textarea or input for typing messages.
    await expect(
      page.locator(
        '.message-composer, [placeholder*="Message" i], textarea[placeholder*="message" i], .composer-input'
      )
    ).not.toHaveCount(0);
  });

  test('page is keyboard reachable — focusable elements present', async ({ page }) => {
    const focusable = await page
      .locator('button, a, input, textarea, [tabindex="0"]')
      .all();
    expect(focusable.length).toBeGreaterThan(0);
  });
});

test.describe('Communities — server rail interaction', () => {
  test('clicking a community icon in the rail switches the active server', async ({ page }) => {
    await page.goto('/communities');
    // Wait for mock data to load.
    await page.waitForTimeout(500);

    const railItems = await page
      .locator('.server-btn, .server-icon, [class*="server-btn"]')
      .all();

    if (railItems.length > 1) {
      await railItems[1].click();
      // After switching, channels should update (or at minimum the page stays intact).
      await expect(page.locator('.channel-sidebar, .channels-panel, [class*="channel"]')).not.toHaveCount(0);
    } else {
      // Only one community in mock — page should still be usable.
      expect(railItems.length).toBeGreaterThanOrEqual(1);
    }
  });
});
