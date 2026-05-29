import { BasePage } from './base.page.js';

/**
 * MarketplacePage — Industrial Magnetite marketplace UI.
 *
 * Layout:
 *   .marketplace > header.marketplace-header
 *                > .filters-section
 *                > .games-grid-section  (game-grid or empty-state)
 *
 * Heading:       <h1 id="marketplace-heading" class="mkt-heading">Discover Rust Games</h1>
 * Game cards:    <div class="game-card"> (via GameCard component)
 * Search:        <input class="search-input"> inside .search-container[role="search"]
 * Categories:    <nav class="category-pills" aria-label="Game categories">
 */
export class MarketplacePage extends BasePage {
  constructor(page) {
    super(page);
    // Game card selector (GameCard component renders .game-card root div)
    this.gameCards = '.game-card';
    // Marketplace h1: "Discover Rust Games"
    this.pageHeading = 'h1#marketplace-heading, h1.mkt-heading';
    // Search input inside header search container
    this.searchInput = '.search-input, input[placeholder*="earch" i]';
    // Category pills nav
    this.categoryNav = 'nav.category-pills, nav[aria-label="Game categories"]';
    // Loading: spinner or role="status"
    this.loadingSpinner = '.loading-spinner, .spinner, [role="status"]';
  }

  async getGameCards() {
    return this.page.locator(this.gameCards).all();
  }

  async getGameCardCount() {
    return this.page.locator(this.gameCards).count();
  }

  async getPageHeading() {
    return this.getText(this.pageHeading);
  }

  async search(query) {
    await this.fill(this.searchInput, query);
    await this.page.keyboard.press('Enter');
  }

  async selectCategory(categoryName) {
    await this.click(`${this.categoryNav} button:has-text("${categoryName}")`);
  }

  async waitForLoading() {
    // Wait for any spinner to disappear; fall back to short timeout if none appears
    try {
      await this.page.waitForSelector(this.loadingSpinner, { state: 'hidden', timeout: 5000 });
    } catch {
      await this.page.waitForTimeout(500);
    }
  }
}
