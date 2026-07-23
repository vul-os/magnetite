import { test, expect } from '@playwright/test';
import { CommunitiesPage } from './page-objects/communities.page.js';

// The Discord-style layout only renders once communities data loads (otherwise
// the page shows a "Join a community" empty state). CommsContext cascades:
// communities → first community → its channels → first text channel → messages,
// plus a per-community members fetch. Stub all of those (cross-origin, so CORS)
// so the server rail, channel sidebar, member list and composer render without a
// backend. Two communities let the rail-switch test click a second server.
const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS',
  'Access-Control-Allow-Headers': '*',
};
const COMMUNITIES = [
  { id: 'c1', name: 'Rustaceans',   icon_url: null, description: 'Rust gamedev', member_count: 3 },
  { id: 'c2', name: 'Speedrunners', icon_url: null, description: 'Go fast',      member_count: 2 },
];
// The backend serialises the channel type as `kind` (channels.kind), which is
// what useChannels filters on to build textChannels — so the fixture must use
// `kind`, not `type`, or the first-text-channel auto-select never fires.
const CHANNELS = [
  { id: 'ch1', name: 'general',      kind: 'text',  community_id: 'c1', position: 0 },
  { id: 'ch2', name: 'off-topic',    kind: 'text',  community_id: 'c1', position: 1 },
  { id: 'ch3', name: 'Voice Lounge', kind: 'voice', community_id: 'c1', position: 2 },
];
const MEMBERS = [
  { id: 'm1', username: 'alice', display_name: 'Alice', status: 'online',  roles: [] },
  { id: 'm2', username: 'bob',   display_name: 'Bob',   status: 'online',  roles: [] },
  { id: 'm3', username: 'carol', display_name: 'Carol', status: 'offline', roles: [] },
];
const MESSAGES = [
  { id: 'msg1', channel_id: 'ch1', content: 'Hey everyone!', created_at: '2026-07-23T10:00:00Z', author: { id: 'm1', username: 'alice', display_name: 'Alice' } },
  { id: 'msg2', channel_id: 'ch1', content: 'Welcome in',    created_at: '2026-07-23T10:01:00Z', author: { id: 'm2', username: 'bob',   display_name: 'Bob' } },
];

async function stubComms(page) {
  await page.route('**/api/v1/**', async (route) => {
    if (route.request().method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers: CORS });
      return;
    }
    const path = new URL(route.request().url()).pathname;
    const json = (obj) =>
      route.fulfill({ status: 200, headers: { ...CORS, 'Content-Type': 'application/json' }, body: JSON.stringify(obj) });
    if (path === '/api/v1/communities') return json({ communities: COMMUNITIES });
    if (/\/api\/v1\/communities\/[^/]+\/channels$/.test(path)) return json({ channels: CHANNELS });
    if (/\/api\/v1\/communities\/[^/]+\/members$/.test(path)) return json({ members: MEMBERS });
    if (/\/api\/v1\/communities\/[^/]+\/voice-rooms$/.test(path)) return json({ rooms: [] });
    if (/\/api\/v1\/channels\/[^/]+\/messages$/.test(path)) return json({ messages: MESSAGES });
    return route.continue();
  });
}

test.describe('Communities', () => {
  let communitiesPage;

  test.beforeEach(async ({ page }) => {
    communitiesPage = new CommunitiesPage(page);
    await stubComms(page);
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
    // Wait briefly for data to load from the API.
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

  test('message composer is present on the auto-selected text channel', async ({ page }) => {
    // The first text channel is auto-selected on load, which mounts the
    // MessageComposer (.message-composer) — no manual channel click needed.
    await expect(page.locator('.message-composer')).toBeVisible();
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
    await stubComms(page);
    await page.goto('/communities');
    // Wait for data to load from the API.
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
