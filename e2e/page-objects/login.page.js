import { BasePage } from './base.page.js';

export class LoginPage extends BasePage {
  constructor(page) {
    super(page);
    // Selectors matching the redesigned Industrial Magnetite login page
    this.emailInput = 'input[type="email"], input[name="email"], input[placeholder*="mail" i]';
    this.passwordInput = 'input[type="password"], input[name="password"]';
    this.submitButton = 'button[type="submit"], .auth-submit-btn';
    this.oauthButtons = '.oauth-btn, [class*="oauth"], a[href*="/api/auth/"]';
    this.errorMessage = '.auth-error, .error-message, [role="alert"]';
    this.pageTitle = 'h1';
  }

  async login(email, password) {
    await this.fill(this.emailInput, email);
    await this.fill(this.passwordInput, password);
    await this.click(this.submitButton);
  }

  async getPageTitle() {
    return this.getText(this.pageTitle);
  }

  async getOAuthButtons() {
    return this.page.locator(this.oauthButtons).all();
  }

  async isOAuthButtonVisible(provider) {
    return this.isVisible(`a[href*="/api/auth/${provider}"], .oauth-btn:has-text("${provider}")`);
  }
}
