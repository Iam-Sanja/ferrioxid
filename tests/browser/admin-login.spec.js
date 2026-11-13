const { test, expect } = require("@playwright/test");

test("native admin login starts from an empty browser profile", async ({ browser }) => {
  const context = await browser.newContext({ storageState: undefined });
  const page = await context.newPage();
  const cookies = await context.cookies();
  expect(cookies.find((cookie) => cookie.name === "ferrioxid_session")).toBeUndefined();
  const response = await page.goto("/admin/login");
  expect(response.status()).toBe(200);
  await expect(page.locator('input[type="password"]')).toBeVisible();
  await context.close();
});
