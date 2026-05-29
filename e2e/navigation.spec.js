import { test, expect } from '@playwright/test';
import { NavigationPage } from './page-objects/navigation.page.js';

test.describe('Navigation', () => {
  let navigationPage;

  test.beforeEach(async ({ page }) => {
    navigationPage = new NavigationPage(page);
    await navigationPage.navigate('/');
  });

  test('navbar links work', async ({ page }) => {
    const links = await navigationPage.getNavbarLinks();
    expect(links.length).toBeGreaterThan(0);
  });

  test('footer links', async ({ page }) => {
    const footerLinks = await navigationPage.getFooterLinks();
    expect(footerLinks.length).toBeGreaterThan(0);
  });
});
