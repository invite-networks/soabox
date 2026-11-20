import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it, vi } from "vitest";
import { exampleZone, userMe } from "../test/fixtures";
import { apiError, json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("zone tabs", () => {
  it("keeps the selected tab in the URL and hides gated tabs", async () => {
    const { user, router } = renderApp("/zones/example.com.");
    await findHeading("example.com.");
    const tabs = screen.getByRole("tablist", { name: "Zone sections" });
    expect(
      within(tabs)
        .getAllByRole("tab")
        .map((t) => t.textContent),
    ).toEqual(["Records", "DNSSEC", "Metadata", "History", "Settings"]);
    await user.click(within(tabs).getByRole("tab", { name: "History" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "history" }));
  });

  it("limits tabs for a user without DNSSEC or history rights", async () => {
    renderApp("/zones/example.com.?tab=dnssec", { me: userMe });
    await findHeading("example.com.");
    const tabs = screen.getByRole("tablist", { name: "Zone sections" });
    expect(
      within(tabs)
        .getAllByRole("tab")
        .map((t) => t.textContent),
    ).toEqual(["Records", "Settings"]);
    expect(within(tabs).getByRole("tab", { name: "Records" })).toHaveAttribute("aria-selected", "true");
  });

  it("manages DNSSEC keys and shows DS records with copy", async () => {
    const calls = recordMutations();
    server.use(
      http.put("/v1/zones/:id/cryptokeys/:key", noContent),
      http.delete("/v1/zones/:id/cryptokeys/:key", noContent),
      http.post("/v1/zones/:id/cryptokeys", () => json({}, 201)),
      http.post("/v1/zones/:id/dnssec", noContent),
    );
    const { user } = renderApp("/zones/example.com.?tab=dnssec");
    await findHeading("example.com.");
    expect(await screen.findByLabelText("DS records")).toHaveTextContent("example.com. IN DS 12345 13 2 ABCDEF");
    await user.click(screen.getByRole("button", { name: "Copy DS records" }));
    expect(await screen.findByText("Copied")).toBeInTheDocument();
    expect(await navigator.clipboard.readText()).toBe("example.com. IN DS 12345 13 2 ABCDEF");

    await user.click(screen.getByRole("button", { name: "Deactivate" }));
    await user.click(screen.getByRole("button", { name: "Unpublish" }));
    await user.click(screen.getByRole("button", { name: "Remove CSK key 1" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove key" });
    await user.click(within(confirm).getByRole("button", { name: "Remove key" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Add key" }));
    const add = await screen.findByRole("dialog", { name: "Add key" });
    await user.type(within(add).getByLabelText("Bits"), "abc");
    await user.click(within(add).getByRole("button", { name: "Add key" }));
    expect(within(add).getByText("Enter a number of bits.")).toBeInTheDocument();
    await user.clear(within(add).getByLabelText("Bits"));
    await user.type(within(add).getByLabelText("Bits"), "256");
    await user.click(within(add).getByRole("button", { name: /Algorithm/ }));
    await user.click(await screen.findByRole("option", { name: "ED25519" }));
    await user.click(within(add).getByRole("button", { name: "Add key" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add key" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Disable DNSSEC" }));
    const disable = await screen.findByRole("alertdialog", { name: "Disable DNSSEC" });
    await user.click(within(disable).getByRole("button", { name: "Disable DNSSEC" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await waitFor(() =>
      expect(calls.map((c) => [c.method, c.path, c.body])).toEqual([
        ["PUT", "/v1/zones/example.com./cryptokeys/1", { active: false, published: true }],
        ["PUT", "/v1/zones/example.com./cryptokeys/1", { active: true, published: false }],
        ["DELETE", "/v1/zones/example.com./cryptokeys/1", undefined],
        [
          "POST",
          "/v1/zones/example.com./cryptokeys",
          { keytype: "csk", active: true, algorithm: "ed25519", bits: 256 },
        ],
        ["POST", "/v1/zones/example.com./dnssec", { enabled: false }],
      ]),
    );
  });

  it("enables DNSSEC on an unsigned zone and shows errors", async () => {
    server.use(
      http.get("/v1/zones/example.com.", () => json({ ...exampleZone, dnssec: false })),
      http.post("/v1/zones/:id/dnssec", () => apiError(502, "UPSTREAM_ERROR", "Signing failed")),
    );
    const { user } = renderApp("/zones/example.com.?tab=dnssec");
    await findHeading("example.com.");
    await user.click(await screen.findByRole("button", { name: "Enable DNSSEC" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Signing failed");
    expect(screen.queryByRole("region", { name: "Keys" })).not.toBeInTheDocument();
  });

  it("views, edits, adds, and deletes metadata", async () => {
    const calls = recordMutations();
    server.use(
      http.put("/v1/zones/:id/metadata/:kind", noContent),
      http.delete("/v1/zones/:id/metadata/:kind", noContent),
    );
    const { user } = renderApp("/zones/example.com.?tab=metadata");
    await findHeading("example.com.");
    const table = await screen.findByRole("table", { name: "Metadata" });
    expect(within(table).getByText("192.0.2.0/24")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Edit ALLOW-AXFR-FROM" }));
    let dialog = await screen.findByRole("dialog", { name: "Edit ALLOW-AXFR-FROM" });
    await user.type(within(dialog).getByLabelText("Values"), "\n198.51.100.0/24");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Add metadata" }));
    dialog = await screen.findByRole("dialog", { name: "Add metadata" });
    await user.type(within(dialog).getByLabelText("Kind"), "bad kind");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(within(dialog).getByText(/Enter a metadata kind/)).toBeInTheDocument();
    await user.clear(within(dialog).getByLabelText("Kind"));
    await user.type(within(dialog).getByLabelText("Kind"), "SOA-EDIT");
    await user.type(within(dialog).getByLabelText("Values"), "INCEPTION-INCREMENT");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Delete ALLOW-AXFR-FROM" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete metadata" });
    await user.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await waitFor(() =>
      expect(calls.map((c) => [c.method, c.path, c.body])).toEqual([
        ["PUT", "/v1/zones/example.com./metadata/ALLOW-AXFR-FROM", { metadata: ["192.0.2.0/24", "198.51.100.0/24"] }],
        ["PUT", "/v1/zones/example.com./metadata/SOA-EDIT", { metadata: ["INCEPTION-INCREMENT"] }],
        ["DELETE", "/v1/zones/example.com./metadata/ALLOW-AXFR-FROM", undefined],
      ]),
    );
  });

  it("shows zone history with a readable diff", async () => {
    const { user } = renderApp("/zones/example.com.?tab=history");
    await findHeading("example.com.");
    const table = await screen.findByRole("table", { name: "Zone history" });
    expect(within(table).queryByRole("columnheader", { name: "Zone" })).not.toBeInTheDocument();
    const toggles = within(table).getAllByRole("button", { name: "Show changes" });
    await user.click(toggles[0] as HTMLElement);
    expect(within(table).getByText("www.example.com. A")).toBeInTheDocument();
    expect(within(table).getByText("192.0.2.1")).toBeInTheDocument();
    expect(within(table).getByText("Removed:", { exact: false })).toBeInTheDocument();
    await user.click(within(table).getAllByRole("button", { name: "Show changes" })[0] as HTMLElement);
    expect(within(table).getByText("Zone already exists")).toBeInTheDocument();
    expect(within(table).getByLabelText("Request")).toHaveTextContent('"kind": "Native"');
    expect(within(table).getByText("Failed (422)")).toBeInTheDocument();
    await user.click(within(table).getAllByRole("button", { name: "Show changes" })[0] as HTMLElement);
    expect(within(table).getByText("kind")).toBeInTheDocument();
    expect(within(table).getByText("Master")).toBeInTheDocument();
    await user.click(within(table).getAllByRole("button", { name: "Hide changes" })[0] as HTMLElement);
  });

  it("saves zone settings and runs zone actions", async () => {
    const calls = recordMutations();
    server.use(
      http.get("/v1/zones/example.com.", () => json({ ...exampleZone, kind: "Slave", masters: ["192.0.2.53"] })),
      http.put("/v1/zones/:id", noContent),
      http.post("/v1/zones/:id/notify", () => json({ result: "Notification queued" })),
      http.post("/v1/zones/:id/rectify", () => json({ result: "Rectified" })),
      http.post("/v1/zones/:id/axfr-retrieve", () => json({ result: "Retrieval queued" })),
      http.get("/v1/zones/:id/export", () => json({ zone: "example.com. 3600 IN SOA ..." })),
    );
    const createObjectURL = vi.fn(() => "blob:zone");
    const revokeObjectURL = vi.fn();
    Object.assign(URL, { createObjectURL, revokeObjectURL });
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    const { user } = renderApp("/zones/example.com.?tab=settings");
    await findHeading("example.com.");

    await user.clear(await screen.findByLabelText("Primary servers"));
    await user.type(screen.getByLabelText("Primary servers"), "bogus");
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(screen.getByText("bogus is not an IP address.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Primary servers"));
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(screen.getByText("Enter at least one primary server address.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Primary servers"), "192.0.2.54");
    await user.click(screen.getByRole("switch", { name: "Rectify on change" }));
    await user.click(within(screen.getByRole("region", { name: "Settings" })).getByRole("button", { name: /Account/ }));
    await user.click(await screen.findByRole("option", { name: "No account" }));
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Zone settings saved.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Send notify" }));
    expect(await screen.findByText("Notification queued")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rectify" }));
    expect(await screen.findByText("Rectified")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retrieve from primary" }));
    expect(await screen.findByText("Retrieval queued")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Export zone file" }));
    await waitFor(() => expect(click).toHaveBeenCalled());
    expect(createObjectURL).toHaveBeenCalled();

    await waitFor(() =>
      expect(calls[0]?.body).toEqual({
        kind: "Slave",
        masters: ["192.0.2.54"],
        account_id: null,
        soa_edit_api: "DEFAULT",
        api_rectify: true,
      }),
    );
    click.mockRestore();
  });

  it("deletes a zone only after typing its name", async () => {
    const calls = recordMutations();
    server.use(http.delete("/v1/zones/:id", noContent));
    const { user } = renderApp("/zones/example.com.?tab=settings");
    await findHeading("example.com.");
    await user.click(await screen.findByRole("button", { name: "Delete zone" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete zone" });
    const button = within(confirm).getByRole("button", { name: "Delete zone" });
    expect(button).toBeDisabled();
    await user.type(within(confirm).getByLabelText("Type example.com. to confirm"), "example.com.");
    expect(button).toBeEnabled();
    await user.click(button);
    await findHeading("Zones");
    expect(calls.map((c) => `${c.method} ${c.path}`)).toEqual(["DELETE /v1/zones/example.com."]);
  });

  it("hides zone deletion without permission and shows action errors", async () => {
    server.use(http.post("/v1/zones/:id/rectify", () => apiError(502, "UPSTREAM_ERROR", "Rectify failed")));
    const { user } = renderApp("/zones/example.com.?tab=settings", { me: userMe });
    await findHeading("example.com.");
    await screen.findByRole("button", { name: "Rectify" });
    expect(screen.queryByRole("button", { name: "Delete zone" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send notify" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rectify" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Rectify failed");
  });
});
