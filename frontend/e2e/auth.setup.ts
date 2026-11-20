import { expect, test as setup } from "@playwright/test";
import { credentials } from "./env";

setup("sign in as admin", async ({ page }) => {
  const { username, password } = credentials();
  await page.goto("/login");
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Dashboard" })).toBeVisible();
  await page.context().storageState({ path: "e2e/.auth/admin.json" });
});
