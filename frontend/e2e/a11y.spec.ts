import { expect, test } from "@playwright/test";
import { expectNoSeriousA11yViolations } from "./axe";
import { uniqueZone } from "./env";

const pages: { path: string; heading: string }[] = [
  { path: "/", heading: "Dashboard" },
  { path: "/zones", heading: "Zones" },
  { path: "/history", heading: "History" },
  { path: "/templates", heading: "Templates" },
  { path: "/api-keys", heading: "API keys" },
  { path: "/server", heading: "Server" },
  { path: "/profile", heading: "Profile" },
  { path: "/admin/users", heading: "Users" },
  { path: "/admin/accounts", heading: "Accounts" },
  { path: "/admin/settings", heading: "Settings" },
  { path: "/admin/tsig", heading: "TSIG keys" },
];

for (const scheme of ["light", "dark"] as const) {
  test.describe(`accessibility in ${scheme} mode`, () => {
    test.use({ colorScheme: scheme });

    for (const { path, heading } of pages) {
      test(`${path} has no serious axe violations`, async ({ page }) => {
        await page.goto(path);
        await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible();
        await expect(page.getByRole("progressbar")).toHaveCount(0);
        await expectNoSeriousA11yViolations(page);
      });
    }
  });
}

test("zone tabs have no serious axe violations", async ({ page }) => {
  const zone = uniqueZone("axe");
  await page.goto("/zones");
  await page.getByRole("button", { name: "Create zone" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create zone" });
  await expectNoSeriousA11yViolations(page);
  await dialog.getByLabel("Zone name").fill(zone);
  await dialog.getByLabel("Name servers").fill(`ns1.${zone}`);
  await dialog.getByRole("button", { name: "Create zone" }).click();
  await expect(page.getByRole("heading", { level: 1, name: zone })).toBeVisible();

  for (const tab of ["records", "dnssec", "metadata", "history", "settings"]) {
    await page.goto(`/zones/${encodeURIComponent(zone)}?tab=${tab}`);
    await expect(page.getByRole("heading", { level: 1, name: zone })).toBeVisible();
    await expect(page.getByRole("progressbar")).toHaveCount(0);
    await expectNoSeriousA11yViolations(page);
  }

  await page.getByRole("button", { name: "Delete zone" }).click();
  const confirm = page.getByRole("alertdialog", { name: "Delete zone" });
  await expectNoSeriousA11yViolations(page);
  await confirm.getByLabel(`Type ${zone} to confirm`).fill(zone);
  await confirm.getByRole("button", { name: "Delete zone" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Zones" })).toBeVisible();
});
