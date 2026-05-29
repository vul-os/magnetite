import { BasePage } from './base.page.js';

export class MarketplacePage extends BasePage {
  constructor(page) {
    super(page);
    this.gameCards = '[data-testid="game-card"]';
    this.pageTitle = 'h1';
    this.searchInput = '[data-testid="search-input"]';
    this.filterButton = '[data-testid="filter-button"]';
    this.loadingSpinner = '[data-testid="loading-spinner"]';
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
    await this.page.waitForSelector(this.loadingSpinner, { state: 'hidden' });
  }
}
