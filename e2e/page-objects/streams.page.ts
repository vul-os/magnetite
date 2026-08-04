import type { Page } from '@playwright/test';
import { BasePage } from './base.page';

/**
 * StreamsPage — Streams browse + Go-Live UI.
 *
 * Layout (per Streams.jsx + Streams.css):
 *   .streams-page > .streams-header   — heading + Go Live button
 *                > .streams-grid      — StreamCard grid
 *                > .stream-player     — when a stream is active
 *                > .go-live-panel     — GoLivePanel overlay when going live
 *
 * Key selectors aligned to real component classes.
 */
export class StreamsPage extends BasePage {
  readonly pageHeading: string;
  readonly goLiveButton: string;
  readonly streamCards: string;
  readonly streamPlayer: string;
  readonly goLivePanel: string;
  readonly viewerCount: string;

  constructor(page: Page) {
    super(page);
    this.pageHeading = 'h1, h2, [class*="streams-title"]';
    this.goLiveButton = 'button:has-text("Go Live"), .go-live-btn, [aria-label*="Go Live" i]';
    this.streamCards = '.stream-card, [class*="stream-card"]';
    this.streamPlayer = '.stream-player, [class*="stream-player"]';
    this.goLivePanel = '.go-live-panel, [class*="go-live"]';
    this.viewerCount = '[class*="viewer"], .viewer-count, :has-text("viewer")';
  }

  async getStreamCardCount(): Promise<number> {
    return this.page.locator(this.streamCards).count();
  }

  async clickGoLive(): Promise<void> {
    await this.page.locator(this.goLiveButton).first().click();
  }

  async isGoLivePanelVisible(): Promise<boolean> {
    return this.page.locator(this.goLivePanel).isVisible();
  }

  async clickFirstStream(): Promise<void> {
    await this.page.locator(this.streamCards).first().click();
  }

  async isPlayerVisible(): Promise<boolean> {
    return this.page.locator(this.streamPlayer).isVisible();
  }
}
