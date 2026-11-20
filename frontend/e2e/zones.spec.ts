import { expect, type Page, test } from "@playwright/test";
import { uniqueZone } from "./env";

async function createZone(page: Page, name: string) {
  await page.goto("/zones");
  await page.getByRole("button", { name: "Create zone" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create zone" });
  await dialog.getByLabel("Zone name").fill(name);
  await dialog.getByLabel("Name servers").fill(`ns1.${name}\nns2.${name}`);
  await dialog.getByRole("button", { name: "Create zone" }).click();
  await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
}

async function deleteZone(page: Page, name: string) {
  await page.goto(`/zones/${encodeURIComponent(name)}?tab=settings`);
  await page.getByRole("button", { name: "Delete zone" }).click();
  const confirm = page.getByRole("alertdialog", { name: "Delete zone" });
  await confirm.getByLabel(`Type ${name} to confirm`).fill(name);
  await confirm.getByRole("button", { name: "Delete zone" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Zones" })).toBeVisible();
}

test.describe("zones", () => {
  test("creates a zone, edits records through one reviewed change, and deletes it", async ({ page }) => {
    const zone = uniqueZone("e2e");
    await createZone(page, zone);

    // Add an A record.
    await page.getByRole("button", { name: "Add record" }).click();
    let dialog = page.getByRole("dialog", { name: "Add records" });
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill("www");
    await dialog.getByRole("textbox", { name: "IPv4 address" }).fill("192.0.2.10");
    await dialog.getByRole("button", { name: "Add to pending changes" }).click();

    // Add a TXT record in the same batch.
    await page.getByRole("button", { name: "Add record" }).click();
    dialog = page.getByRole("dialog", { name: "Add records" });
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill("@");
    await dialog.getByRole("button", { name: /Type/ }).click();
    await page.getByRole("option", { name: "TXT", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Text" }).fill("v=spf1 -all");
    await dialog.getByRole("button", { name: "Add to pending changes" }).click();

    // Add a CNAME with a relative target. The zone is appended.
    await page.getByRole("button", { name: "Add record" }).click();
    dialog = page.getByRole("dialog", { name: "Add records" });
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill("alias");
    await dialog.getByRole("button", { name: /Type/ }).click();
    await page.getByRole("option", { name: "CNAME", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Target" }).fill("www");
    await expect(dialog).toContainText(`.${zone}`);
    await dialog.getByRole("button", { name: "Add to pending changes" }).click();

    const pending = page.getByRole("region", { name: "Pending changes" });
    await expect(pending).toContainText("3 pending changes");

    const patches: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "PATCH") {
        patches.push(request.url());
      }
    });
    await pending.getByRole("button", { name: "Review and apply" }).click();
    const review = page.getByRole("dialog", { name: "Review pending changes" });
    await expect(review).toContainText("192.0.2.10");
    await review.getByRole("button", { name: "Apply 3 changes" }).click();
    await expect(page.getByText("Changes applied.")).toBeVisible();
    expect(patches).toHaveLength(1);

    const records = page.getByRole("table", { name: "Records" });
    await expect(records.getByRole("rowheader", { name: "www" })).toBeVisible();
    await expect(records).toContainText('"v=spf1 -all"');
    await expect(records).toContainText(`www.${zone}`);

    // Edit the record, then confirm history recorded the change.
    await page.getByRole("button", { name: "Edit www A" }).click();
    dialog = page.getByRole("dialog", { name: "Edit A records" });
    await dialog.getByRole("textbox", { name: "IPv4 address" }).fill("192.0.2.11");
    await dialog.getByRole("button", { name: "Save to pending changes" }).click();
    await page.getByRole("button", { name: "Review and apply" }).click();
    await page.getByRole("button", { name: "Apply 1 change" }).click();
    await expect(page.getByText("Changes applied.")).toBeVisible();
    await expect(records).toContainText("192.0.2.11");

    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByRole("table", { name: "Zone history" })).toBeVisible();

    await deleteZone(page, zone);
  });

  test("reloading a deep link keeps the page", async ({ page }) => {
    await page.goto("/admin/settings");
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
  });
});
