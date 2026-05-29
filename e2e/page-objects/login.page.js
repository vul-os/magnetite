import { BasePage } from './base.page.js';

/**
 * LoginPage — Industrial Magnetite split-panel auth UI.
 *
 * Layout:
 *   .auth-split > .auth-hero  (left pitch panel)
 *               > .auth-form-panel > .auth-form-inner
 *
 * Form panel heading: <h1 class="auth-title">Welcome back</h1>
 * Submit button:      <button class="auth-submit-btn">Sign In</button>
 * OAuth buttons:      <button class="oauth-btn" aria-label="Continue with <Provider>">
 * Error container:    <div class="auth-error" role="alert">
 */
export class LoginPage extends BasePage {
  constructor(page) {
    super(page);
    // Form-panel selectors (Industrial Magnetite auth redesign)
    this.emailInput = 'input[type="email"], input[name="email"], input[placeholder*="mail" i]';
    this.passwordInput = 'input[type="password"], input[name="password"]';
    this.submitButton = 'button.auth-submit-btn';
    // OAuth buttons rendered by OAuthButtons component with aria-label="Continue with <Provider>"
    this.oauthButtons = 'button.oauth-btn';
    // Error: class="auth-error" + role="alert"
    this.errorMessage = '.auth-error[role="alert"], [role="alert"].auth-error';
    // Form panel h1 ("Welcome back")
    this.formHeading = '.auth-title';
  }

  async login(email, password) {
    await this.fill(this.emailInput, email);
    await this.fill(this.passwordInput, password);
    await this.click(this.submitButton);
  }

  async getFormHeading() {
    return this.getText(this.formHeading);
  }

  async getOAuthButtons() {
    return this.page.locator(this.oauthButtons).all();
  }

  async isOAuthButtonVisible(provider) {
    // Buttons use aria-label="Continue with <Provider>"
    return this.isVisible(`button.oauth-btn[aria-label*="${provider}"]`);
  }

  async submitEmpty() {
    await this.click(this.submitButton);
  }

  async isErrorVisible() {
    return this.isVisible(this.errorMessage);
  }
}
