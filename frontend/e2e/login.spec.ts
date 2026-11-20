import { expect, test } from "@playwright/test";
import { expectNoSeriousA11yViolations } from "./axe";
import { credentials } from "./env";

test.describe("sign in", () => {
  test("deep links redirect to sign in and back", async ({ page }) => {
    const { username, password } = credentials();
    await page.goto("/zones");
    await expect(page).toHaveURL(/\/login\?redirect=/);
    await expect(page.getByRole("heading", { level: 1, name: "Sign in to SOAbox" })).toBeVisible();
    await expectNoSeriousA11yViolations(page);

    await page.getByLabel("Username").fill(username);
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Zones" })).toBeVisible();
    await expect(page).toHaveURL(/\/zones$/);
  });

  test("rejects a wrong password", async ({ page }) => {
    const { username } = credentials();
    await page.goto("/login");
    await page.getByLabel("Username").fill(username);
    await page.getByLabel("Password").fill("definitely not the password");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page).toHaveURL(/\/login/);
  });

  test("shows single sign-on callback errors", async ({ page }) => {
    await page.goto("/login?error=oidc_failed");
    await expect(page.getByRole("alert")).toContainText("Single sign-on did not complete");
  });
});
