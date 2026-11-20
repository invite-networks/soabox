import { screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { beforeEach, describe, expect, it } from "vitest";
import type { LoginRequest } from "../api/types";
import { adminMe } from "../test/fixtures";
import { apiError, json } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { server } from "../test/server";

describe("sign in", () => {
  beforeEach(() => {
    server.use(http.get("/v1/me", () => apiError(401, "UNAUTHENTICATED", "Sign in required")));
  });

  it("redirects to sign in and returns to the requested page", async () => {
    server.use(http.post("/auth/login", () => json(adminMe)));
    const { user, router } = renderApp("/zones");
    await findHeading("Sign in to SOAbox");
    expect(router.state.location.search).toMatchObject({ redirect: "/zones" });
    await user.type(await screen.findByLabelText("Username"), "admin");
    await user.type(screen.getByLabelText("Password"), "correct horse battery");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await findHeading("Zones");
  });

  it("validates empty fields and shows bad credential errors", async () => {
    server.use(http.post("/auth/login", () => apiError(401, "UNAUTHENTICATED", "Invalid username or password.")));
    const { user } = renderApp("/login");
    await findHeading("Sign in to SOAbox");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("Enter your username.")).toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toHaveFocus();
    await user.type(screen.getByLabelText("Username"), "admin");
    await user.type(screen.getByLabelText("Password"), "wrong");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Invalid username or password.");
  });

  it("asks for a TOTP code when required", async () => {
    const bodies: LoginRequest[] = [];
    server.use(
      http.post("/auth/login", async ({ request }) => {
        const body = (await request.json()) as LoginRequest;
        bodies.push(body);
        if (!body.totp_code) {
          return apiError(401, "TOTP_REQUIRED", "Enter your code.");
        }
        if (body.totp_code !== "123456") {
          return apiError(401, "UNAUTHENTICATED", "That code is not valid.");
        }
        return json(adminMe);
      }),
    );
    const { user } = renderApp("/login");
    await findHeading("Sign in to SOAbox");
    await user.type(await screen.findByLabelText("Username"), "admin");
    await user.type(screen.getByLabelText("Password"), "secret password");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    const code = await screen.findByLabelText("Authentication code");
    await user.type(code, "12");
    await user.click(screen.getByRole("button", { name: "Verify" }));
    await waitFor(() => expect(code).toHaveAccessibleDescription(/Enter the code from your authenticator app/));
    await user.clear(code);
    await user.type(code, "000000");
    await user.click(screen.getByRole("button", { name: "Verify" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That code is not valid.");
    await user.clear(screen.getByLabelText("Authentication code"));
    await user.type(screen.getByLabelText("Authentication code"), "123456");
    await user.click(screen.getByRole("button", { name: "Verify" }));
    await findHeading("Dashboard");
    expect(bodies.at(-1)).toEqual({ username: "admin", password: "secret password", totp_code: "123456" });
  });

  it("can go back from the TOTP step", async () => {
    server.use(http.post("/auth/login", () => apiError(401, "TOTP_REQUIRED", "Enter your code.")));
    const { user } = renderApp("/login");
    await findHeading("Sign in to SOAbox");
    await user.type(await screen.findByLabelText("Username"), "admin");
    await user.type(screen.getByLabelText("Password"), "x");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await screen.findByLabelText("Authentication code");
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByLabelText("Username")).toBeInTheDocument();
  });

  it("shows single sign-on and callback errors", async () => {
    renderApp("/login?error=pending_approval");
    await findHeading("Sign in to SOAbox");
    expect(screen.getByRole("alert")).toHaveTextContent("waiting for an administrator");
    const sso = await screen.findByRole("link", { name: "Sign in with Example ID" });
    expect(sso).toHaveAttribute("href", "/auth/oidc/start");
  });

  it("hides the password form when local sign-in is off", async () => {
    server.use(http.get("/auth/config", () => json({ local_enabled: false, oidc_enabled: true, oidc_label: "" })));
    renderApp("/login");
    expect(await screen.findByRole("link", { name: "Single sign-on" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Username")).not.toBeInTheDocument();
  });

  it("shows an error when sign-in options cannot load", async () => {
    server.use(http.get("/auth/config", () => HttpResponse.error()));
    renderApp("/login");
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("could not be reached"), { timeout: 4000 });
  });
});
