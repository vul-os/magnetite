import { BasePage } from './base.page.js';

/**
 * LoginPage — Industrial Magnetite split-panel auth UI.
 *
 * Layout:
 *   .auth-split > .auth-hero  (left pitch panel)
 *               > .auth-form-panel > .auth-form-inner
 *
 * Form panel heading: <h1 class="auth-title">Welcome back</h1>
 * Submit button:      <button class="auth-submit">Sign In</button>
 * OAuth buttons:      <button class="oauth-btn" aria-label="Continue with <Provider>">
 * Error container:    <div class="auth-alert" role="alert">
 */
export class LoginPage extends BasePage {
  constructor(page) {
    super(page);
    // Form-panel selectors (Industrial Magnetite auth redesign)
    this.emailInput = 'input[type="email"], input[name="email"], input[placeholder*="mail" i]';
    this.passwordInput = 'input[type="password"], input[name="password"]';
    this.submitButton = 'button.auth-submit';
    // OAuth buttons rendered by OAuthButtons component with aria-label="Continue with <Provider>"
    this.oauthButtons = 'button.oauth-btn';
    // Error: class="auth-alert" + role="alert"
    this.errorMessage = '.auth-alert[role="alert"], [role="alert"].auth-alert';
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
    // .all() takes a synchronous snapshot and does not auto-wait, so wait for
    // the async-rendered OAuth section to appear before counting — otherwise
    // this races the SPA's first client render and returns an empty array.
    await this.page.locator(this.oauthButtons).first().waitFor({ state: 'visible' });
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
