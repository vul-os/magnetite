import { test, expect } from '@playwright/test';
import { LoginPage } from './page-objects/login.page.js';

test.describe('Auth', () => {
  let loginPage;

  test.beforeEach(async ({ page }) => {
    loginPage = new LoginPage(page);
    await loginPage.navigate('/login');
  });

  // Login page — Industrial Magnetite split-panel layout
  // h1 in the form panel reads "Welcome back"; hero panel also has an h1
  test('login page renders with correct heading', async ({ page }) => {
    // The form panel heading is "Welcome back" (auth-title)
    await expect(page.getByRole('heading', { name: /welcome back/i })).toBeVisible();
  });

  test('OAuth buttons exist with correct providers', async ({ page }) => {
    // OAuthButtons renders buttons with aria-label "Continue with <Provider>"
    await expect(page.getByRole('button', { name: /continue with google/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /continue with discord/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /continue with github/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /continue with gitlab/i })).toBeVisible();
  });

  test('OAuth buttons count via page object', async ({ page: _page }) => {
    const oauthButtons = await loginPage.getOAuthButtons();
    expect(oauthButtons.length).toBeGreaterThan(0);
  });

  test('form validation — shows error on empty submit', async ({ page }) => {
    // Submit button has class auth-submit; a rejected sign-in renders the
    // error container <div class="auth-alert" role="alert">.
    await page.click('button.auth-submit');
    await expect(page.locator('[role="alert"].auth-alert')).toBeVisible();
  });

  test('sign-in button present', async ({ page }) => {
    await expect(page.getByRole('button', { name: /sign in/i })).toBeVisible();
  });
});

test.describe('Auth — Register', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/register');
  });

  // Register page heading is "Join Magnetite"; submit button is "Create Account"
  test('register page renders with correct heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /join magnetite/i })).toBeVisible();
  });

  test('create account button present', async ({ page }) => {
    await expect(page.getByRole('button', { name: /create account/i })).toBeVisible();
  });

  test('register OAuth buttons present', async ({ page }) => {
    await expect(page.getByRole('button', { name: /continue with google/i })).toBeVisible();
  });
});
