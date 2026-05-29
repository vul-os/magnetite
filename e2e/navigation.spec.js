import { test, expect } from '@playwright/test';
import { NavigationPage } from './page-objects/navigation.page.js';

test.describe('Navigation', () => {
  let navigationPage;

  test.beforeEach(async ({ page }) => {
    navigationPage = new NavigationPage(page);
    await navigationPage.navigate('/');
  });

  // Navbar is <nav className="navbar ..."> — selector is nav.navbar
  test('navbar links present', async ({ page: _page }) => {
    const links = await navigationPage.getNavbarLinks();
    expect(links.length).toBeGreaterThan(0);
  });

  // Footer is <footer className="footer"> — selector is footer.footer
  test('footer links present', async ({ page: _page }) => {
    const footerLinks = await navigationPage.getFooterLinks();
    expect(footerLinks.length).toBeGreaterThan(0);
  });

  test('navbar logo visible', async ({ page }) => {
    // Logo uses class .navbar-logo
    await expect(page.locator('.navbar-logo')).toBeVisible();
  });

  test('marketplace link navigates', async ({ page }) => {
    await navigationPage.clickNavbarLink('Marketplace');
    await expect(page).toHaveURL(/marketplace/);
  });

  test('home page has hero heading', async ({ page }) => {
    // Landing page HeroSection has a prominent h1/h2 heading
    await expect(page.locator('h1, h2').first()).toBeVisible();
  });
});
