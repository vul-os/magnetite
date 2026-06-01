/**
 * e2e/game-studio.spec.js
 *
 * End-to-end spec for the "Create Game in Studio" flow.
 *
 * Route: /developers/studio  (lazy-loaded GameStudio page)
 *
 * Tests cover:
 *  1. Page navigation + basic structure
 *  2. GitHub integration UI (connect / error states)
 *  3. Game configuration form (field editing, category options)
 *  4. Deploy button state (disabled until GitHub connected + title filled)
 *  5. Deploy game flow (mock API responses via page.route)
 *  6. A11y: landmarks, aria roles, visible headings
 */

import { test, expect } from '@playwright/test';

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Navigate to the Studio page with an auth token set in localStorage
 * (so the page doesn't redirect to /login).
 */
async function goToStudio(page) {
  // Set a fake JWT so useAuth doesn't redirect.
  await page.addInitScript(() => {
    localStorage.setItem(
      'token',
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyLTEiLCJlbWFpbCI6ImRldkBleGFtcGxlLmNvbSIsImV4cCI6OTk5OTk5OTk5OX0.FAKE'
    );
  });

  // Stub the GitHub installations check so the page starts unconnected.
  await page.route('**/api/github/installations', (route) => {
    route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: JSON.stringify({ message: 'Unauthorized' }),
    });
  });

  // Stub api.games.create so we can control the deploy response.
  await page.route('**/api/v1/games', (route) => {
    if (route.request().method() === 'POST') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ data: { id: 'game-e2e-001', title: 'E2E Test Game' } }),
      });
    } else {
      route.continue();
    }
  });

  await page.goto('/developers/studio');
  // Wait for the lazy-loaded page to finish rendering.
  await page.waitForSelector('.game-studio', { timeout: 15000 });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

test.describe('Game Studio — page structure', () => {
  test.beforeEach(async ({ page }) => {
    await goToStudio(page);
  });

  test('page title heading is visible', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /game studio/i })).toBeVisible();
  });

  test('kicker label "RUST GAME STUDIO" is visible', async ({ page }) => {
    await expect(page.getByText(/rust game studio/i)).toBeVisible();
  });

  test('Step 1 — GitHub Integration section is visible', async ({ page }) => {
    await expect(page.getByText(/github integration/i)).toBeVisible();
    await expect(page.getByText(/step 1/i)).toBeVisible();
  });

  test('Step 2 — Game Configuration section is visible', async ({ page }) => {
    await expect(page.getByText(/game configuration/i)).toBeVisible();
    await expect(page.getByText(/step 2/i)).toBeVisible();
  });

  test('Connect GitHub button is present when not connected', async ({ page }) => {
    await expect(page.getByRole('button', { name: /connect github/i })).toBeVisible();
  });

  test('GitHub repo input is present', async ({ page }) => {
    await expect(
      page.getByPlaceholder(/owner\/repository/i)
    ).toBeVisible();
  });

  test('Game Title field is present', async ({ page }) => {
    await expect(page.getByLabel(/game title/i)).toBeVisible();
  });

  test('Category selector is present', async ({ page }) => {
    await expect(page.getByLabel(/category/i)).toBeVisible();
  });

  test('Min Players and Max Players fields are present', async ({ page }) => {
    await expect(page.getByLabel(/min players/i)).toBeVisible();
    await expect(page.getByLabel(/max players/i)).toBeVisible();
  });

  test('Thumbnail URL field is present', async ({ page }) => {
    await expect(page.getByLabel(/thumbnail url/i)).toBeVisible();
  });

  test('Deploy Game button is initially disabled', async ({ page }) => {
    await expect(page.getByRole('button', { name: /deploy game/i })).toBeDisabled();
  });

  test('permissions list is visible', async ({ page }) => {
    await expect(page.getByText(/permissions granted/i)).toBeVisible();
  });
});

test.describe('Game Studio — GitHub connect', () => {
  test.beforeEach(async ({ page }) => {
    await goToStudio(page);
  });

  test('typing a valid repo enables the connect button', async ({ page }) => {
    const input = page.getByPlaceholder(/owner\/repository/i);
    await input.fill('acme/my-shooter');
    await expect(page.getByRole('button', { name: /connect github/i })).toBeEnabled();
  });

  test('submit with invalid repo format shows error', async ({ page }) => {
    // Stub the register endpoint to return 400.
    await page.route('**/api/v1/github/repos/register', (route) => {
      route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ message: 'Registration failed (HTTP 400)' }),
      });
    });

    const input = page.getByPlaceholder(/owner\/repository/i);
    await input.fill('noslash');
    await page.getByRole('button', { name: /connect github/i }).click();

    // Error is shown in role="alert" container.
    await expect(page.getByRole('alert').first()).toBeVisible();
  });

  test('successful repo registration shows connected state', async ({ page }) => {
    // Stub the register endpoint to return success.
    await page.route('**/api/v1/github/repos/register', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ data: { id: 1, full_name: 'acme/my-shooter' } }),
      });
    });

    const input = page.getByPlaceholder(/owner\/repository/i);
    await input.fill('acme/my-shooter');
    await page.getByRole('button', { name: /connect github/i }).click();

    await expect(page.getByText(/connected/i)).toBeVisible({ timeout: 5000 });
    await expect(page.getByRole('button', { name: /disconnect/i })).toBeVisible();
  });

  test('Disconnect button resets to connect state', async ({ page }) => {
    // First connect via the installations endpoint.
    await page.route('**/api/github/installations', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ installations: [{ id: 1, full_name: 'acme/game' }] }),
      });
    });

    await page.goto('/developers/studio');
    await page.waitForSelector('.game-studio', { timeout: 15000 });

    await expect(page.getByRole('button', { name: /disconnect/i })).toBeVisible({ timeout: 5000 });
    await page.getByRole('button', { name: /disconnect/i }).click();
    await expect(page.getByRole('button', { name: /connect github/i })).toBeVisible();
  });
});

test.describe('Game Studio — game configuration', () => {
  test.beforeEach(async ({ page }) => {
    await goToStudio(page);
  });

  test('Category selector includes all expected options', async ({ page }) => {
    const select = page.getByLabel(/category/i);
    const options = await select.evaluate((el) =>
      Array.from(el.options).map((o) => o.value)
    );
    expect(options).toContain('Action');
    expect(options).toContain('Puzzle');
    expect(options).toContain('RPG');
    expect(options).toContain('Strategy');
  });

  test('editing the title field persists the value', async ({ page }) => {
    const titleInput = page.getByLabel(/game title/i);
    await titleInput.fill('Neon Arena');
    await expect(titleInput).toHaveValue('Neon Arena');
  });

  test('editing the description field persists the value', async ({ page }) => {
    const desc = page.getByLabel(/description/i);
    await desc.fill('A fast-paced arena shooter.');
    await expect(desc).toHaveValue('A fast-paced arena shooter.');
  });

  test('Deploy button requires a title to not be disabled (when GitHub connected)', async ({ page }) => {
    // Connect via installations stub.
    await page.route('**/api/github/installations', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ installations: [{ id: 1, full_name: 'acme/game' }] }),
      });
    });

    await page.goto('/developers/studio');
    await page.waitForSelector('.game-studio', { timeout: 15000 });

    // Wait for connected state.
    await expect(page.getByRole('button', { name: /disconnect/i })).toBeVisible({ timeout: 5000 });

    // Still disabled — title is empty.
    await expect(page.getByRole('button', { name: /deploy game/i })).toBeDisabled();

    // Fill title.
    await page.getByLabel(/game title/i).fill('My Cool Game');

    // Should now be enabled.
    await expect(page.getByRole('button', { name: /deploy game/i })).toBeEnabled({ timeout: 3000 });
  });
});

test.describe('Game Studio — create game in studio (deploy flow)', () => {
  async function connectAndFill(page) {
    // Stub installations as connected.
    await page.route('**/api/github/installations', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ installations: [{ id: 1, full_name: 'acme/my-game' }] }),
      });
    });

    // Stub games/create.
    await page.route('**/api/v1/games', (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: { id: 'game-e2e-001', title: 'Space Shooter' } }),
        });
      } else {
        route.continue();
      }
    });

    await page.addInitScript(() => {
      localStorage.setItem(
        'token',
        'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyLTEiLCJlbWFpbCI6ImRldkBleGFtcGxlLmNvbSIsImV4cCI6OTk5OTk5OTk5OX0.FAKE'
      );
    });

    await page.goto('/developers/studio');
    await page.waitForSelector('.game-studio', { timeout: 15000 });

    // Wait for GitHub connected state.
    await expect(page.getByRole('button', { name: /disconnect/i })).toBeVisible({ timeout: 8000 });

    // Fill form.
    await page.getByLabel(/game title/i).fill('Space Shooter');
    await page.getByLabel(/description/i).fill('A server-authoritative space shooter.');
    await page.getByLabel(/price/i).fill('0.50');
  }

  test('full create-game flow shows Deployed! on success', async ({ page }) => {
    await connectAndFill(page);

    await page.getByRole('button', { name: /deploy game/i }).click();

    await expect(page.getByText(/deployed/i)).toBeVisible({ timeout: 5000 });
  });

  test('deploy button shows deploying spinner while in-flight', async ({ page }) => {
    // Add a delay to the mock so we can observe the loading state.
    await page.route('**/api/v1/games', async (route) => {
      if (route.request().method() === 'POST') {
        await new Promise((r) => setTimeout(r, 800));
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: { id: 'g-2' } }),
        });
      } else {
        route.continue();
      }
    });

    await connectAndFill(page);
    await page.getByRole('button', { name: /deploy game/i }).click();

    await expect(page.getByText(/deploying/i)).toBeVisible({ timeout: 3000 });
  });

  test('deploy shows error alert when API fails', async ({ page }) => {
    await page.route('**/api/v1/games', (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'Internal server error' }),
        });
      } else {
        route.continue();
      }
    });

    await connectAndFill(page);
    await page.getByRole('button', { name: /deploy game/i }).click();

    await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 5000 });
  });

  test('POST to /api/v1/games includes title and category in payload', async ({ page }) => {
    const requestBodies = [];
    await page.route('**/api/v1/games', (route) => {
      if (route.request().method() === 'POST') {
        requestBodies.push(route.request().postDataJSON());
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ data: { id: 'g-3' } }),
        });
      } else {
        route.continue();
      }
    });

    await connectAndFill(page);
    // Change category to Racing
    await page.getByLabel(/category/i).selectOption('Racing');
    await page.getByRole('button', { name: /deploy game/i }).click();

    await expect(page.getByText(/deployed/i)).toBeVisible({ timeout: 5000 });

    expect(requestBodies).toHaveLength(1);
    expect(requestBodies[0].title).toBe('Space Shooter');
    expect(requestBodies[0].category).toBe('Racing');
  });
});

test.describe('Game Studio — accessibility', () => {
  test.beforeEach(async ({ page }) => {
    await goToStudio(page);
  });

  test('page has a main landmark or layout container', async ({ page }) => {
    // The page wraps content in a layout; at minimum a div with game-studio class.
    await expect(page.locator('.game-studio')).toBeVisible();
  });

  test('form labels are associated with their inputs', async ({ page }) => {
    // Playwright can check accessible labels via getByLabel.
    await expect(page.getByLabel(/game title/i)).toBeVisible();
    await expect(page.getByLabel(/category/i)).toBeVisible();
    await expect(page.getByLabel(/min players/i)).toBeVisible();
  });

  test('GitHub repo input has an accessible label', async ({ page }) => {
    await expect(
      page.getByRole('textbox', { name: /github repository/i })
    ).toBeVisible();
  });

  test('no focusable element without an accessible name (buttons have labels)', async ({ page }) => {
    const buttons = page.getByRole('button');
    const count = await buttons.count();
    expect(count).toBeGreaterThan(0);
    for (let i = 0; i < count; i++) {
      const btn = buttons.nth(i);
      const name = await btn.getAttribute('aria-label');
      const text = await btn.textContent();
      expect(
        (name && name.trim().length > 0) || (text && text.trim().length > 0),
        `Button at index ${i} must have a label or text`
      ).toBe(true);
    }
  });
});
