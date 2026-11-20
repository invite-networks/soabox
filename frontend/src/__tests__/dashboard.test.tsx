import { screen } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { dashboard } from "../test/fixtures";
import { json } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { server } from "../test/server";

describe("dashboard", () => {
  it("shows zone count, server info, and recent changes", async () => {
    renderApp("/");
    await findHeading("Dashboard");
    expect(await screen.findByText("5.0.1")).toBeInTheDocument();
    expect(screen.getByText("1d 2h")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Browse zones" })).toHaveAttribute("href", "/zones");
    expect(screen.getByRole("table", { name: "Recent changes" })).toBeInTheDocument();
  });

  it("shows unknown uptime when PowerDNS does not report it and handles no server", async () => {
    server.use(http.get("/v1/dashboard", () => json({ ...dashboard, server: { version: "5.0.1", uptime: null } })));
    const first = renderApp("/");
    expect(await screen.findByText("5.0.1")).toBeInTheDocument();
    expect(screen.getByText("Unknown")).toBeInTheDocument();
    first.unmount();
    server.use(http.get("/v1/dashboard", () => json({ ...dashboard, server: null, recent_history: [] })));
    renderApp("/");
    expect(await screen.findByText("No changes yet")).toBeInTheDocument();
    expect(screen.getByText("Server did not respond")).toBeInTheDocument();
  });
});
