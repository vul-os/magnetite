import type { Page } from '@playwright/test';
import { BasePage } from './base.page';

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
  readonly pageHeading: string;
  readonly kicker: string;
  readonly statusBar: string;
  readonly bindingRows: string;
  readonly resetButton: string;
  readonly bindButton: string;
  readonly axesSection: string;
  readonly buttonsSection: string;

  constructor(page: Page) {
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

  async getBindingRowCount(): Promise<number> {
    return this.page.locator(this.bindingRows).count();
  }

  async clickReset(): Promise<void> {
    await this.page.locator(this.resetButton).first().click();
  }

  async isHeadingVisible(): Promise<boolean> {
    return this.page.locator(this.pageHeading).isVisible();
  }
}
