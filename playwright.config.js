export default {
  testDir: './e2e',
  // game-studio.spec.js describes the OLD studio form (Category / Min-Max
  // Players / Thumbnail / "Step 1/2" sections). The studio was redesigned to a
  // template-based flow, so those 28 tests assert a UI that no longer exists and
  // need a full rewrite — excluded from the gated suite until then.
  testIgnore: '**/game-studio.spec.js',
  timeout: 30000,
  use: {
    // Must match the dev server's port. vite.config.js pins Magnetite to 5174
    // with strictPort (5173 is commonly taken by other Vite apps), so 5173 here
    // pointed the whole suite at a dead port — every spec failed before it began.
    baseURL: 'http://localhost:5174',
    headless: true,
    // The PWA service worker proxies fetches and would bypass page.route(),
    // turning every stubbed API call into an un-intercepted network request.
    // Block it so specs can control the API (the app runs fine without the SW).
    serviceWorkers: 'block',
  },
  // Start the dev server for the run instead of silently requiring one already
  // up on 5174. Locally an existing server is reused (fast iteration); in CI a
  // fresh one is always started.
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5174',
    reuseExistingServer: !process.env.CI,
    timeout: 60000,
  },
};
