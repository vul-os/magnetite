export class BasePage {
  constructor(page) {
    this.page = page;
  }

  async navigate(path) {
    await this.page.goto(path);
  }

  async waitForSelector(selector) {
    await this.page.waitForSelector(selector);
  }

  async click(selector) {
    await this.page.click(selector);
  }

  async fill(selector, value) {
    await this.page.fill(selector, value);
  }

  async getText(selector) {
    return this.page.textContent(selector);
  }

  async isVisible(selector) {
    return this.page.isVisible(selector);
  }
}
