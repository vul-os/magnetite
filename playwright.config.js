export default {
  testDir: './e2e',
  timeout: 30000,
  use: {
    // Must match the dev server's port. vite.config.js pins Magnetite to 5174
    // with strictPort (5173 is commonly taken by other Vite apps), so 5173 here
    // pointed the whole suite at a dead port — every spec failed before it began.
    baseURL: 'http://localhost:5174',
    headless: true,
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
