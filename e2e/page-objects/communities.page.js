import { BasePage } from './base.page.js';

/**
 * CommunitiesPage — Discord-like communities experience.
 *
 * Layout (per Communities.jsx + Communities.css):
 *   .communities-layout
 *     .server-rail           — community icon list (ServerRail)
 *     .channel-sidebar       — ChannelList + VoicePanel
 *     .main-chat             — MessageList + MessageComposer
 *     .member-list           — MemberList
 *
 * Key selectors aligned to the real component classes.
 */
export class CommunitiesPage extends BasePage {
  constructor(page) {
    super(page);
    this.serverRail = '.server-rail, [data-testid="server-rail"]';
    this.channelSidebar = '.channel-sidebar, .channels-panel';
    this.mainChat = '.main-chat, .chat-area';
    this.memberList = '.member-list, .members-panel';
    this.messageComposer = '.message-composer, .composer, [placeholder*="Message" i], textarea[placeholder*="message" i]';
    this.chatMessages = '.message-list .message, .msg-row, .chat-message';
    this.pageHeading = 'h1, h2';
    this.connectionPill = '.connection-pill, [data-testid="connection-pill"]';
  }

  async getServerRail() {
    return this.page.locator(this.serverRail);
  }

  async getChannelItems() {
    return this.page.locator(`${this.channelSidebar} .channel-item, .channel-btn`).all();
  }

  async getMemberItems() {
    return this.page.locator(`${this.memberList} .member-item, .member-row`).all();
  }

  async typeMessage(text) {
    const composer = this.page.locator(this.messageComposer).first();
    await composer.fill(text);
  }

  async getChatMessageCount() {
    return this.page.locator(this.chatMessages).count();
  }

  async isLayoutVisible() {
    return this.page.locator('.communities-layout, .communities-page').isVisible();
  }
}
