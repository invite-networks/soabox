import { defineConfig, devices } from "@playwright/test";

/**
 * End-to-end tests run against a real SOAbox backend serving the built UI.
 *
 * SOABOX_E2E_BASE_URL    where SOAbox listens (default http://localhost:8080)
 * SOABOX_E2E_USERNAME    a local Admin user
 * SOABOX_E2E_PASSWORD    that user's password
 */
const baseURL = process.env["SOABOX_E2E_BASE_URL"] ?? "http://localhost:8080";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  workers: 1,
  reporter: process.env["CI"] ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL,
    trace: "retain-on-failure",
  },
  projects: [
    { name: "setup", testMatch: /auth\.setup\.ts/ },
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], storageState: "e2e/.auth/admin.json" },
      dependencies: ["setup"],
      testIgnore: /login\.spec\.ts/,
    },
    {
      name: "signed-out",
      use: { ...devices["Desktop Chrome"] },
      testMatch: /login\.spec\.ts/,
    },
  ],
});
