import { BasePage } from './base.page.js';

/**
 * ControllerSettingsPage — Gamepad controller binding editor.
 *
 * Layout (per ControllerSettings.jsx + ControllerSettings.css):
 *   .controller-settings-page
 *     .cs-header           — heading + kicker
 *     .cs-status-bar       — connected / not-connected status
 *     .cs-bindings-table   — action → current binding rows
 *     .cs-axes-panel       — live axis visualization
 *     .cs-buttons-panel    — live button state
 *
 * Key selectors aligned to real component classes.
 */
export class ControllerSettingsPage extends BasePage {
  constructor(page) {
    super(page);
    this.pageHeading = 'h1, [class*="controller"] h1, [class*="cs-title"]';
    this.kicker = '[class*="kicker"], [class*="cs-kicker"]';
    this.statusBar = '[class*="status"], .cs-status-bar, [class*="connected"]';
    this.bindingRows = '.cs-binding-row, [class*="binding-row"]';
    this.resetButton = 'button:has-text("Reset"), [class*="reset"]';
    this.bindButton = 'button:has-text("Bind"), button:has-text("Press"), [class*="listen"]';
    this.axesSection = '[class*="axes"], .cs-axes-panel';
    this.buttonsSection = '[class*="buttons"], .cs-buttons-panel';
  }

  async getBindingRowCount() {
    return this.page.locator(this.bindingRows).count();
  }

  async clickReset() {
    await this.page.locator(this.resetButton).first().click();
  }

  async isHeadingVisible() {
    return this.page.locator(this.pageHeading).isVisible();
  }
}
