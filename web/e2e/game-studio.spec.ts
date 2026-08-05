import { test, expect } from '@playwright/test';

/**
 * Game Studio — /developers/studio
 *
 * The page is a three-step flow: template gallery → configure → result.
 * api.templates.list() falls back to built-in templates on error, so the
 * gallery always renders without a backend, and the page needs no auth token —
 * so these tests drive the real UI directly, no stubbing required.
 */

test.describe('Game Studio — template gallery', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/developers/studio');
    await page.waitForSelector('.game-studio');
  });

  test('page heading and kicker are visible', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /game studio/i, level: 1 })).toBeVisible();
    await expect(page.getByText(/rust game studio/i)).toBeVisible();
  });

  test('the step indicator is present', async ({ page }) => {
    await expect(page.locator('.studio-steps')).toBeVisible();
  });

  test('the template gallery renders template cards', async ({ page }) => {
    await expect(page.locator('.template-gallery')).toBeVisible();
    const cards = page.locator('.template-card');
    await expect(cards.first()).toBeVisible();
    expect(await cards.count()).toBeGreaterThan(0);
  });
});

test.describe('Game Studio — configure step', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/developers/studio');
    await page.waitForSelector('.game-studio');
    // Pick the first template to advance from the gallery to the configure step.
    await page.locator('.template-card').first().click();
  });

  test('selecting a template reveals the configure form', async ({ page }) => {
    await expect(page.locator('.configure-form')).toBeVisible();
    await expect(page.locator('#game-name')).toBeVisible();
  });

  test('name and description fields are present', async ({ page }) => {
    await expect(page.locator('#game-name')).toBeVisible();
    await expect(page.locator('#game-desc')).toBeVisible();
  });

  test('the name field is labelled (accessible)', async ({ page }) => {
    await expect(page.locator('label[for="game-name"]')).toBeVisible();
  });

  test('the create button is disabled until a name is entered', async ({ page }) => {
    const submit = page.locator('.configure-form button[type="submit"]');
    await expect(submit).toBeDisabled();
    await page.locator('#game-name').fill('My Test Game');
    await expect(submit).toBeEnabled();
  });

  test('editing the name field persists the value', async ({ page }) => {
    const name = page.locator('#game-name');
    await name.fill('Voxel Arena');
    await expect(name).toHaveValue('Voxel Arena');
  });

  test('the back button returns to the template gallery', async ({ page }) => {
    await page.locator('.back-btn').click();
    await expect(page.locator('.template-gallery')).toBeVisible();
  });
});
