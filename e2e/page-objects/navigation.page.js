import { BasePage } from './base.page.js';

/**
 * NavigationPage — Industrial Magnetite Navbar + Footer.
 *
 * Navbar: <nav className="navbar [scrolled]">
 *           <div className="navbar-container">
 *             <div className="navbar-left">
 *               <Link className="navbar-logo">  ← logo
 *             <div className="navbar-nav">      ← desktop links
 *             <div className="navbar-right">    ← auth/account links
 *
 * Footer:  <footer className="footer">
 *
 * Mobile: hamburger button with aria-label="Toggle menu" (class .navbar-menu-btn).
 */
export class NavigationPage extends BasePage {
  constructor(page) {
    super(page);
    // Primary nav links are inside nav.navbar (desktop) — use this selector so
    // mobile menu links are not double-counted when viewport is large.
    this.navbarLinks = 'nav.navbar a';
    this.footerLinks = 'footer.footer a';
    this.logo = '.navbar-logo';
    this.mobileMenuButton = '.navbar-menu-btn, [aria-label="Toggle menu"]';
    // Mobile menu overlay
    this.mobileMenuOpen = '.navbar-mobile-open, .mobile-menu[aria-expanded="true"]';
  }

  async getNavbarLinks() {
    // .all() is a synchronous snapshot with no auto-wait, so wait for the navbar
    // to render before counting — otherwise this races the SPA's first client
    // render and returns an empty array.
    await this.page.locator(this.navbarLinks).first().waitFor({ state: 'visible' });
    return this.page.locator(this.navbarLinks).all();
  }

  async getFooterLinks() {
    await this.page.locator(this.footerLinks).first().waitFor({ state: 'visible' });
    return this.page.locator(this.footerLinks).all();
  }

  async clickNavbarLink(text) {
    await this.click(`nav.navbar a:has-text("${text}")`);
  }

  async clickFooterLink(text) {
    await this.click(`footer.footer a:has-text("${text}")`);
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
