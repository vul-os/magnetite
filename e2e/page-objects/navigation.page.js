import { BasePage } from './base.page.js';

export class NavigationPage extends BasePage {
  constructor(page) {
    super(page);
    // Selectors matching the redesigned Industrial Magnetite navbar/footer
    this.navbarLinks = 'nav.navbar a';
    this.footerLinks = 'footer.footer a';
    this.logo = '.navbar-logo';
    this.mobileMenuButton = '.navbar-menu-btn, [aria-label="Toggle menu"]';
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
