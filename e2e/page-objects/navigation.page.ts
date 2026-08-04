import type { Locator, Page } from '@playwright/test';
import { BasePage } from './base.page';

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
  readonly navbarLinks: string;
  readonly footerLinks: string;
  readonly logo: string;
  readonly mobileMenuButton: string;
  readonly mobileMenuOpen: string;

  constructor(page: Page) {
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

  async getNavbarLinks(): Promise<Locator[]> {
    // .all() is a synchronous snapshot with no auto-wait, so wait for the navbar
    // to render before counting — otherwise this races the SPA's first client
    // render and returns an empty array.
    await this.page.locator(this.navbarLinks).first().waitFor({ state: 'visible' });
    return this.page.locator(this.navbarLinks).all();
  }

  async getFooterLinks(): Promise<Locator[]> {
    await this.page.locator(this.footerLinks).first().waitFor({ state: 'visible' });
    return this.page.locator(this.footerLinks).all();
  }

  async clickNavbarLink(text: string): Promise<void> {
    await this.click(`nav.navbar a:has-text("${text}")`);
  }

  async clickFooterLink(text: string): Promise<void> {
    await this.click(`footer.footer a:has-text("${text}")`);
  }

  async isLogoVisible(): Promise<boolean> {
    return this.isVisible(this.logo);
  }

  async openMobileMenu(): Promise<void> {
    if (await this.isVisible(this.mobileMenuButton)) {
      await this.click(this.mobileMenuButton);
    }
  }
}
