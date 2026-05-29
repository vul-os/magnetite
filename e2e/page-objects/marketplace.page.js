import { BasePage } from './base.page.js';

export class MarketplacePage extends BasePage {
  constructor(page) {
    super(page);
    // Selectors matching the redesigned Industrial Magnetite marketplace
    this.gameCards = '.game-card';
    this.pageTitle = 'h1';
    this.searchInput = '.search-bar-input, input[type="search"], input[placeholder*="earch"]';
    this.filterButton = '.filter-btn, [aria-label*="filter"], [aria-label*="Filter"]';
    this.loadingSpinner = '.loading-spinner, .spinner, [role="status"]';
  }

  async getGameCards() {
    return this.page.locator(this.gameCards).all();
  }

  async getGameCardCount() {
    return this.page.locator(this.gameCards).count();
  }

  async getPageTitle() {
    return this.getText(this.pageTitle);
  }

  async search(query) {
    await this.fill(this.searchInput, query);
    await this.page.keyboard.press('Enter');
  }

  async waitForLoading() {
    // Wait for spinner to disappear, or fall back to a short wait if no spinner
    try {
      await this.page.waitForSelector(this.loadingSpinner, { state: 'hidden', timeout: 5000 });
    } catch {
      await this.page.waitForTimeout(500);
    }
  }
}
