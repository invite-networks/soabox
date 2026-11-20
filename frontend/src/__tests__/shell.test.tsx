import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { userMe } from "../test/fixtures";
import { apiError } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("app shell", () => {
  it("shows every destination once for an admin and marks the current page", async () => {
    renderApp("/zones");
    await findHeading("Zones");
    const nav = screen.getByRole("navigation", { name: "Main" });
    const names = within(nav)
      .getAllByRole("link")
      .map((l) => l.textContent);
    expect(names).toEqual([
      "Dashboard",
      "Zones",
      "History",
      "Templates",
      "API keys",
      "Server",
      "Users",
      "Accounts",
      "Settings",
      "TSIG keys",
    ]);
    expect(within(nav).getByRole("link", { name: "Zones" })).toHaveAttribute("aria-current", "page");
  });

  it("hides destinations the user cannot use and blocks direct visits", async () => {
    renderApp("/admin/users", { me: userMe });
    await findHeading("Users");
    expect(screen.getByText("You do not have permission to view this page")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).queryByRole("link", { name: "Users" })).not.toBeInTheDocument();
    expect(within(nav).queryByRole("link", { name: "History" })).not.toBeInTheDocument();
    expect(within(nav).getByRole("link", { name: "API keys" })).toBeInTheDocument();
  });

  it("moves focus to the page heading after navigation", async () => {
    const { user } = renderApp("/");
    await findHeading("Dashboard");
    const nav = screen.getByRole("navigation", { name: "Main" });
    await user.click(within(nav).getByRole("link", { name: "Templates" }));
    const heading = await findHeading("Templates");
    await waitFor(() => expect(heading).toHaveFocus());
  });

  it("switches theme and signs out from the user menu", async () => {
    const calls = recordMutations();
    const { user } = renderApp("/");
    await findHeading("Dashboard");
    await user.click(screen.getByRole("button", { name: "Account menu for Admin" }));
    await user.click(await screen.findByRole("menuitem", { name: /Dark theme/ }));
    expect(document.documentElement.dataset["theme"]).toBe("dark");
    expect(window.localStorage.getItem("soabox-theme")).toBe("dark");

    server.use(http.get("/v1/me", () => apiError(401, "UNAUTHENTICATED", "Signed out")));
    await user.click(screen.getByRole("button", { name: "Account menu for Admin" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    await findHeading("Sign in to SOAbox");
    expect(calls.map((c) => `${c.method} ${c.path}`)).toContain("POST /auth/logout");
  });

  it("opens the profile from the user menu", async () => {
    const { user } = renderApp("/");
    await findHeading("Dashboard");
    await user.click(screen.getByRole("button", { name: "Account menu for Admin" }));
    await user.click(await screen.findByRole("menuitem", { name: "Profile" }));
    await findHeading("Profile");
  });

  it("sends the user to sign in when a query returns 401", async () => {
    server.use(http.get("/v1/dashboard", () => apiError(401, "UNAUTHENTICATED", "Session expired")));
    const { router } = renderApp("/");
    await findHeading("Sign in to SOAbox");
    expect(router.state.location.pathname).toBe("/login");
  });

  it("finds a zone from the top bar", async () => {
    const { user } = renderApp("/");
    await findHeading("Dashboard");
    await user.type(screen.getByRole("combobox", { name: "Find a zone" }), "org");
    await user.click(await screen.findByRole("option", { name: "example.org." }));
    await findHeading("example.org.");
  });

  it("opens navigation in a drawer on small screens", async () => {
    const { user } = renderApp("/");
    await findHeading("Dashboard");
    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    const drawer = await screen.findByRole("dialog", { name: "Navigation" });
    await user.click(within(drawer).getByRole("link", { name: "Zones" }));
    await findHeading("Zones");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Navigation" })).not.toBeInTheDocument());
  });

  it("renders not found for unknown paths", async () => {
    renderApp("/nope");
    await findHeading("Page not found");
  });
});
