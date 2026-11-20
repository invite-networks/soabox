import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { adminMe } from "../test/fixtures";
import { apiError, json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("server", () => {
  it("shows version, statistics with a filter, and configuration", async () => {
    const { user, router } = renderApp("/server");
    await findHeading("Server");
    expect(await screen.findByText("authoritative")).toBeInTheDocument();
    const stats = await screen.findByRole("table", { name: "Statistics" });
    expect(within(stats).getByText("A=10")).toBeInTheDocument();
    await user.type(screen.getByRole("searchbox", { name: "Filter statistics" }), "udp");
    await waitFor(() => expect(within(stats).getAllByRole("row")).toHaveLength(2));
    await user.type(screen.getByRole("searchbox", { name: "Filter statistics" }), "zzz");
    expect(await screen.findByText("Nothing matches the filter")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Configuration" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "configuration" }));
    const config = await screen.findByRole("table", { name: "Configuration" });
    expect(within(config).getByText("local-port")).toBeInTheDocument();
  });

  it("shows upstream errors", async () => {
    server.use(http.get("/v1/server", () => apiError(503, "UPSTREAM_UNAVAILABLE", "PowerDNS is not reachable")));
    renderApp("/server");
    expect(await screen.findByRole("alert", {}, { timeout: 4000 })).toHaveTextContent("PowerDNS is not reachable");
  });
});

describe("profile", () => {
  it("updates details and changes the password", async () => {
    const calls = recordMutations();
    server.use(
      http.put("/v1/me", () => json({ ...adminMe, user: { ...adminMe.user, display_name: "Root" } })),
      http.put("/v1/me/password", noContent),
    );
    const { user } = renderApp("/profile");
    await findHeading("Profile");
    const name = screen.getByLabelText("Display name");
    await user.clear(name);
    await user.type(name, "Root");
    await user.clear(screen.getByLabelText("Email"));
    await user.type(screen.getByLabelText("Email"), "not-an-email");
    await user.click(screen.getByRole("button", { name: "Save details" }));
    expect(await screen.findByText("Enter a valid email address.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Email"));
    await user.click(screen.getByRole("button", { name: "Save details" }));
    expect(await screen.findByText("Your details were saved.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Account menu for Root" })).toBeInTheDocument();

    await user.type(screen.getByLabelText("Current password"), "old password");
    await user.type(screen.getByLabelText("New password"), "new password 123");
    await user.type(screen.getByLabelText("Confirm new password"), "different");
    await user.click(screen.getByRole("button", { name: "Change password" }));
    expect(await screen.findByText("The passwords do not match.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Confirm new password"));
    await user.type(screen.getByLabelText("Confirm new password"), "new password 123");
    await user.click(screen.getByRole("button", { name: "Change password" }));
    expect(await screen.findByText("Your password was changed.")).toBeInTheDocument();

    expect(calls.map((c) => c.body)).toEqual([
      { display_name: "Root", email: "" },
      { current_password: "old password", new_password: "new password 123" },
    ]);
  });

  it("sets up and enables TOTP", async () => {
    const calls = recordMutations();
    let enabled = false;
    server.use(
      http.get("/v1/me", () => json({ ...adminMe, user: { ...adminMe.user, totp_enabled: enabled } })),
      http.post("/v1/me/totp/setup", () =>
        json({ secret: "JBSWY3DPEHPK3PXP", otpauth_url: "otpauth://totp/SOAbox:admin?secret=JBSWY3DPEHPK3PXP" }),
      ),
      http.post("/v1/me/totp/enable", () => {
        enabled = true;
        return noContent();
      }),
    );
    const { user } = renderApp("/profile");
    await findHeading("Profile");
    await user.click(screen.getByRole("button", { name: "Set up two-factor" }));
    expect(await screen.findByText("JBSWY3DPEHPK3PXP")).toBeInTheDocument();
    expect(screen.getByLabelText("Setup link")).toHaveTextContent("otpauth://totp/SOAbox:admin");
    await user.click(screen.getByRole("button", { name: "Turn on two-factor" }));
    expect(screen.getByText("Enter the code from your authenticator app.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Code from your app"), "123456");
    await user.click(screen.getByRole("button", { name: "Turn on two-factor" }));
    expect(await screen.findByRole("button", { name: "Turn off two-factor" })).toBeInTheDocument();
    expect(calls.map((c) => [c.path, c.body])).toEqual([
      ["/v1/me/totp/setup", undefined],
      ["/v1/me/totp/enable", { code: "123456" }],
    ]);
  });

  it("disables TOTP and hides password sections for single sign-on users", async () => {
    server.use(
      http.get("/v1/me", () => json({ ...adminMe, user: { ...adminMe.user, totp_enabled: true } })),
      http.delete("/v1/me/totp", async ({ request }) => {
        sent = await request.json();
        return apiError(422, "VALIDATION_ERROR", "Wrong code");
      }),
    );
    let sent: unknown;
    const { user, unmount } = renderApp("/profile");
    await findHeading("Profile");
    const form = screen.getByRole("form", { name: "Turn off two-factor authentication" });
    await user.type(within(form).getByLabelText("Current code"), "654321");
    await user.click(within(form).getByRole("button", { name: "Turn off two-factor" }));
    expect(await within(form).findByText("Enter your current password.")).toBeInTheDocument();
    await user.type(within(form).getByLabelText("Current password"), "correct horse battery");
    await user.click(within(form).getByRole("button", { name: "Turn off two-factor" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong code");
    expect(sent).toEqual({ code: "654321", password: "correct horse battery" });
    unmount();

    renderApp("/profile", { me: { ...adminMe, user: { ...adminMe.user, auth_source: "oidc" } } });
    await findHeading("Profile");
    expect(screen.queryByRole("region", { name: "Password" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Two-factor authentication" })).not.toBeInTheDocument();
  });
});
