import { BasePage } from './base.page.js';

export class NavigationPage extends BasePage {
  constructor(page) {
    super(page);
    this.navbarLinks = '[data-testid="navbar"] a';
    this.footerLinks = '[data-testid="footer"] a';
    this.logo = '[data-testid="navbar-logo"]';
    this.mobileMenuButton = '[data-testid="mobile-menu-button"]';
  }

  async getNavbarLinks() {
    return this.page.locator(this.navbarLinks).all();
  }

  async getFooterLinks() {
    return this.page.locator(this.footerLinks).all();
  }

  async clickNavbarLink(text) {
    await this.click(`nav a:has-text("${text}")`);
  }

  async clickFooterLink(text) {
    await this.click(`footer a:has-text("${text}")`);
  }

  async isLogoVisible() {
    return this.isVisible(this.logo);
  }

  async openMobileMenu() {
    if (await this.isVisible(this.mobileMenuButton)) {
      await this.click(this.mobileMenuButton);
    }
  }
}
