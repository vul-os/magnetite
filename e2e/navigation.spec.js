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
    // Two .navbar-logo links exist (desktop + mobile navs); the desktop one is
    // first in the DOM and visible at the default 1280px viewport.
    await expect(page.locator('.navbar-logo').first()).toBeVisible();
  });

  test('clicking a navbar link navigates', async ({ page }) => {
    // "Marketplace" is the root route (/), so it can't be used to assert a URL
    // change from /. Communities → /communities does change the URL.
    await navigationPage.clickNavbarLink('Communities');
    await expect(page).toHaveURL(/\/communities/);
  });

  test('home page has hero heading', async ({ page }) => {
    // Landing page HeroSection has a prominent h1/h2 heading
    await expect(page.locator('h1, h2').first()).toBeVisible();
  });
});
