import { screen, waitFor, within } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { describe, expect, it } from "vitest";
import { exampleZone, userMe, zoneSummaries } from "../test/fixtures";
import { apiError, json } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("zones list", () => {
  it("lists zones with kind, serial, DNSSEC state, and account", async () => {
    renderApp("/zones");
    await findHeading("Zones");
    const table = await screen.findByRole("table", { name: "Zones" });
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(4);
    expect(within(rows[1] as HTMLElement).getByRole("link", { name: "example.com." })).toHaveAttribute(
      "href",
      "/zones/example.com.",
    );
    expect(within(rows[1] as HTMLElement).getByText("Signed")).toBeInTheDocument();
    expect(within(rows[1] as HTMLElement).getByText("Ops")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("Primary")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("Unsigned")).toBeInTheDocument();
    expect(screen.getByText("3 zones")).toBeInTheDocument();
  });

  it("keeps the search in the URL", async () => {
    const { user, router } = renderApp("/zones");
    await screen.findByRole("table", { name: "Zones" });
    await user.type(screen.getByRole("searchbox", { name: "Search zones" }), "org");
    await waitFor(() => expect(router.state.location.search).toEqual({ q: "org" }));
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
  });

  it("paginates with the page in the URL", async () => {
    server.use(http.get("/v1/zones", () => json({ items: zoneSummaries, total: 120 })));
    const { user, router } = renderApp("/zones");
    await screen.findByText("120 zones, page 1 of 3");
    await user.click(screen.getByRole("button", { name: "Next" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ page: 2 }));
    expect(await screen.findByText("120 zones, page 2 of 3")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Previous" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ page: 1 }));
  });

  it("shows empty and error states", async () => {
    server.use(http.get("/v1/zones", () => json({ items: [], total: 0 })));
    const first = renderApp("/zones");
    expect(await screen.findByText("No zones yet")).toBeInTheDocument();
    first.unmount();
    server.use(http.get("/v1/zones", () => apiError(502, "UPSTREAM_ERROR", "PowerDNS returned an error")));
    renderApp("/zones?q=zzz");
    expect(await screen.findByRole("alert", {}, { timeout: 4000 })).toHaveTextContent("PowerDNS returned an error");
  });

  it("hides zone creation without permission", async () => {
    renderApp("/zones", { me: userMe });
    await screen.findByRole("table", { name: "Zones" });
    expect(screen.queryByRole("button", { name: "Create zone" })).not.toBeInTheDocument();
  });

  it("creates a zone from a template and opens it", async () => {
    const calls = recordMutations();
    const created = { ...exampleZone, id: "new.example.", name: "new.example.", rrsets: [] };
    server.use(
      http.post("/v1/zones", () => json(created, 201)),
      http.get("/v1/zones/new.example.", () => json(created)),
    );
    const { user } = renderApp("/zones");
    await findHeading("Zones");
    await user.click(screen.getByRole("button", { name: "Create zone" }));
    const dialog = await screen.findByRole("dialog", { name: "Create zone" });
    await user.click(within(dialog).getByRole("button", { name: "Create zone" }));
    expect(await within(dialog).findByText(/Enter a valid zone name/)).toBeInTheDocument();

    await user.type(within(dialog).getByLabelText("Zone name"), "new.example");
    await user.type(within(dialog).getByLabelText("Name servers"), "ns1.example.com\nns2.example.com.");
    await user.click(within(dialog).getByRole("button", { name: /Template/ }));
    await user.click(await screen.findByRole("option", { name: "Basic web" }));
    await user.click(within(dialog).getByRole("button", { name: /Account/ }));
    await user.click(await screen.findByRole("option", { name: "Web" }));
    await user.click(within(dialog).getByRole("button", { name: /SOA-EDIT-API/ }));
    await user.click(await screen.findByRole("option", { name: "INCREASE" }));
    await user.click(within(dialog).getByRole("button", { name: "Create zone" }));

    await findHeading("new.example.");
    expect(calls).toHaveLength(1);
    expect(calls[0]?.body).toEqual({
      name: "new.example.",
      kind: "Native",
      masters: [],
      nameservers: ["ns1.example.com.", "ns2.example.com."],
      template_id: 1,
      account_id: 2,
      soa_edit_api: "INCREASE",
    });
    expect(calls[0]?.csrf).toBe("csrf-test-token");
  });

  it("requires primary servers for a secondary zone and shows server errors", async () => {
    server.use(http.post("/v1/zones", () => apiError(409, "CONFLICT", "Zone already exists")));
    const { user } = renderApp("/zones");
    await findHeading("Zones");
    await user.click(screen.getByRole("button", { name: "Create zone" }));
    const dialog = await screen.findByRole("dialog", { name: "Create zone" });
    await user.type(within(dialog).getByLabelText("Zone name"), "sec.example");
    await user.click(within(dialog).getByRole("button", { name: /Kind/ }));
    await user.click(await screen.findByRole("option", { name: "Secondary" }));
    await user.click(within(dialog).getByRole("button", { name: "Create zone" }));
    expect(await within(dialog).findByText("Enter at least one primary server address.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Primary servers"), "not-an-ip");
    await user.click(within(dialog).getByRole("button", { name: "Create zone" }));
    expect(await within(dialog).findByText("not-an-ip is not an IP address.")).toBeInTheDocument();
    await user.clear(within(dialog).getByLabelText("Primary servers"));
    await user.type(within(dialog).getByLabelText("Primary servers"), "192.0.2.53:5300");
    await user.click(within(dialog).getByRole("button", { name: "Create zone" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Zone already exists");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("reports a network failure while loading a zone", async () => {
    server.use(http.get("/v1/zones/:id", () => HttpResponse.error()));
    renderApp("/zones/example.com.");
    expect(await screen.findByRole("alert", {}, { timeout: 4000 })).toHaveTextContent("could not be reached");
  });

  it("shows not found for an unknown zone", async () => {
    renderApp("/zones/missing.example.");
    await findHeading("Zone not found");
  });
});
