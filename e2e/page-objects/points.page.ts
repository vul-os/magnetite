import type { Locator, Page } from '@playwright/test';
import { BasePage } from './base.page';

/**
 * PointsPage — Points / score economy dashboard.
 *
 * Layout (per Points.jsx + Points.css):
 *   .points-page > .points-hero        — balance + season card
 *                > .points-tabs        — Balance / History / Rewards / Leaderboard tabs
 *                > .points-content     — tab panel content
 *
 * Key selectors aligned to the real component classes.
 */
export class PointsPage extends BasePage {
  readonly pageHeading: string;
  readonly balanceValue: string;
  readonly seasonCard: string;
  readonly tabBar: string;
  readonly historyTab: string;
  readonly rewardsTab: string;
  readonly leaderboardTab: string;
  readonly rewardCards: string;
  readonly leaderboardRows: string;
  readonly historyRows: string;

  constructor(page: Page) {
    super(page);
    this.pageHeading = 'h1, .pts-kicker + h1, [class*="points-title"]';
    this.balanceValue = '.pts-balance-val, [class*="balance-val"], [class*="points-amount"]';
    this.seasonCard = '.pts-season-card, .season-info, [class*="season"]';
    this.tabBar = '[role="tablist"], .points-tabs, .tab-bar';
    this.historyTab = '[role="tab"]:has-text("History"), button:has-text("History")';
    this.rewardsTab = '[role="tab"]:has-text("Rewards"), button:has-text("Rewards")';
    this.leaderboardTab = '[role="tab"]:has-text("Leaderboard"), button:has-text("Leaderboard")';
    this.rewardCards = '.reward-card, .pts-reward-card, [class*="reward"]';
    this.leaderboardRows = '.lb-row, .leaderboard-entry, [class*="leader"]';
    this.historyRows = '.hist-row, .history-entry, [class*="history"]';
  }

  async getTabBar(): Promise<Locator> {
    return this.page.locator(this.tabBar);
  }

  async clickTab(name: string): Promise<void> {
    await this.page.locator(`button:has-text("${name}")`).first().click();
  }

  async getRewardCount(): Promise<number> {
    return this.page.locator(this.rewardCards).count();
  }

  async getLeaderboardCount(): Promise<number> {
    return this.page.locator(this.leaderboardRows).count();
  }

  async getHistoryCount(): Promise<number> {
    return this.page.locator(this.historyRows).count();
  }
}
