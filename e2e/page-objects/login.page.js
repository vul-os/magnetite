import { BasePage } from './base.page.js';

export class LoginPage extends BasePage {
  constructor(page) {
    super(page);
    this.emailInput = '[data-testid="email-input"]';
    this.passwordInput = '[data-testid="password-input"]';
    this.submitButton = '[data-testid="login-submit"]';
    this.oauthButtons = '[data-testid*="oauth-"]';
    this.errorMessage = '[data-testid="error-message"]';
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
    return this.isVisible(`[data-testid="oauth-${provider}"]`);
  }
}
