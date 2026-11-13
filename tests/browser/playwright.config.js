const { defineConfig } = require("@playwright/test");

module.exports = defineConfig({
  testDir: ".",
  timeout: 30_000,
  use: {
    baseURL: process.env.FEXU_BROWSER_BASE_URL || "http://127.0.0.1:4173",
    ignoreHTTPSErrors: true,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
