import { test, expect } from '@playwright/test';
import { LoginPage } from './page-objects/login.page.js';

test.describe('Auth', () => {
  let loginPage;

  test.beforeEach(async ({ page }) => {
    loginPage = new LoginPage(page);
    await loginPage.navigate('/login');
  });

  test('login page renders', async ({ page }) => {
    await expect(page.locator('h1')).toBeVisible();
  });

  test('OAuth buttons exist', async ({ page }) => {
    const oauthButtons = await loginPage.getOAuthButtons();
    expect(oauthButtons.length).toBeGreaterThan(0);
  });

  test('form validation', async ({ page }) => {
    await page.click('[data-testid="login-submit"]');
    await expect(page.locator('[data-testid="error-message"]')).toBeVisible();
  });
});
