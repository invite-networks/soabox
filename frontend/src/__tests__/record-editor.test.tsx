import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { exampleZone, userMe } from "../test/fixtures";
import { apiError, json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

async function openZone() {
  const app = renderApp("/zones/example.com.");
  await findHeading("example.com.");
  await screen.findByRole("table", { name: "Records" });
  return app;
}

function row(name: string) {
  const table = screen.getByRole("table", { name: "Records" });
  const header = within(table).getByRole("rowheader", { name });
  return header.closest("tr") as HTMLElement;
}

describe("record editor", () => {
  it("collects add, edit, and delete as pending changes and applies them in one PATCH", async () => {
    const calls = recordMutations();
    server.use(http.patch("/v1/zones/:id/rrsets", noContent));
    const { user } = await openZone();

    // Add a new RRset with two values.
    await user.click(screen.getByRole("button", { name: "Add record" }));
    let dialog = await screen.findByRole("dialog", { name: "Add records" });
    await user.type(within(dialog).getByLabelText("Name"), "api");
    await user.type(within(dialog).getByRole("textbox", { name: "IPv4 address" }), "192.0.2.20");
    await user.click(within(dialog).getByRole("button", { name: "Add value" }));
    await user.type(within(dialog).getByRole("textbox", { name: "IPv4 address 2" }), "192.0.2.21");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(within(row("api")).getByText("Added")).toBeInTheDocument();

    // Edit the TTL of www and disable it.
    await user.click(screen.getByRole("button", { name: "Edit www A" }));
    dialog = await screen.findByRole("dialog", { name: "Edit A records" });
    const ttl = within(dialog).getByLabelText("TTL (seconds)");
    await user.clear(ttl);
    await user.type(ttl, "60");
    await user.click(within(dialog).getByRole("checkbox", { name: "Disabled" }));
    await user.click(within(dialog).getByRole("button", { name: "Save to pending changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(within(row("www")).getByText("Changed")).toBeInTheDocument();

    // Delete old.
    await user.click(screen.getByRole("button", { name: "Delete old A" }));
    expect(within(row("old")).getByText("Deleted")).toBeInTheDocument();

    // Nothing has been sent yet.
    expect(calls).toHaveLength(0);
    const panel = screen.getByRole("region", { name: "Pending changes" });
    expect(panel).toHaveTextContent("3 pending changes");

    // Review shows before and after, then apply sends exactly one PATCH.
    await user.click(within(panel).getByRole("button", { name: "Review and apply" }));
    const review = await screen.findByRole("dialog", { name: "Review pending changes" });
    expect(within(review).getByText("api A")).toBeInTheDocument();
    expect(within(review).getByText("192.0.2.20")).toBeInTheDocument();
    expect(within(review).getByText("192.0.2.10 (disabled)")).toBeInTheDocument();
    expect(within(review).getByText("TTL 60")).toBeInTheDocument();
    await user.click(within(review).getByRole("button", { name: "Apply 3 changes" }));

    expect(await screen.findByText("Changes applied.")).toBeInTheDocument();
    expect(calls).toHaveLength(1);
    expect(calls[0]?.method).toBe("PATCH");
    expect(calls[0]?.path).toBe("/v1/zones/example.com./rrsets");
    expect(calls[0]?.csrf).toBe("csrf-test-token");
    await waitFor(() =>
      expect(calls[0]?.body).toEqual({
        rrsets: [
          {
            name: "api.example.com.",
            type: "A",
            ttl: 3600,
            changetype: "REPLACE",
            records: [
              { content: "192.0.2.20", disabled: false },
              { content: "192.0.2.21", disabled: false },
            ],
          },
          { name: "old.example.com.", type: "A", ttl: 300, changetype: "DELETE", records: [] },
          {
            name: "www.example.com.",
            type: "A",
            ttl: 60,
            changetype: "REPLACE",
            records: [{ content: "192.0.2.10", disabled: true }],
          },
        ],
      }),
    );
    expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
  });

  it("validates each field per type", async () => {
    const { user } = await openZone();
    await user.click(screen.getByRole("button", { name: "Add record" }));
    const dialog = await screen.findByRole("dialog", { name: "Add records" });
    const address = within(dialog).getByRole("textbox", { name: "IPv4 address" });
    await user.type(address, "abc999.1.1.1");
    expect(address).toHaveValue("999.1.1.1");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    expect(await within(dialog).findByText(/Enter an IPv4 address/)).toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByRole("textbox", { name: "IPv4 address" })).toHaveFocus());

    await user.click(within(dialog).getByRole("button", { name: /Type/ }));
    await user.click(await screen.findByRole("option", { name: "MX" }));
    const priority = within(dialog).getByRole("textbox", { name: "Priority" });
    await user.type(priority, "7x0000");
    expect(priority).toHaveValue("70000");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    expect(await within(dialog).findByText("Enter a number from 0 to 65535.")).toBeInTheDocument();
    expect(within(dialog).getByText("Mail server is required.")).toBeInTheDocument();

    await user.clear(within(dialog).getByLabelText("Name"));
    await user.type(within(dialog).getByLabelText("Name"), "x.other.net.");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    expect(await within(dialog).findByText("The name must be inside example.com.")).toBeInTheDocument();
  });

  it("appends the zone to relative host names and shows the suffix", async () => {
    const calls = recordMutations();
    server.use(http.patch("/v1/zones/:id/rrsets", noContent));
    const { user } = await openZone();
    await user.click(screen.getByRole("button", { name: "Add record" }));
    const dialog = await screen.findByRole("dialog", { name: "Add records" });
    await user.click(within(dialog).getByRole("button", { name: /Type/ }));
    await user.click(await screen.findByRole("option", { name: "MX" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Priority" }), "10");
    await user.type(within(dialog).getByRole("textbox", { name: "Mail server" }), "mail");
    expect(within(dialog).getByText(".example.com.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add value" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Priority 2" }), "20");
    await user.type(within(dialog).getByRole("textbox", { name: "Mail server 2" }), "mx.other.net.");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const apex = within(screen.getByRole("table", { name: "Records" })).getAllByRole("rowheader", { name: "@" });
    const mx = apex.map((h) => h.closest("tr") as HTMLElement).find((r) => within(r).queryByText("MX"));
    expect(mx).toBeDefined();
    const suffix = within(mx as HTMLElement).getByText(".example.com.");
    expect(suffix).toHaveClass("text-fg-muted");
    expect(within(mx as HTMLElement).getByText("10 mail")).toBeInTheDocument();
    expect(within(mx as HTMLElement).getByText("20 mx.other.net.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Review and apply" }));
    const review = await screen.findByRole("dialog", { name: "Review pending changes" });
    await user.click(within(review).getByRole("button", { name: "Apply 1 change" }));
    await waitFor(() =>
      expect(calls[0]?.body).toEqual({
        rrsets: [
          {
            name: "example.com.",
            type: "MX",
            ttl: 3600,
            changetype: "REPLACE",
            records: [
              { content: "10 mail.example.com.", disabled: false },
              { content: "20 mx.other.net.", disabled: false },
            ],
          },
        ],
      }),
    );
  });

  it("edits the SOA as separate fields", async () => {
    const calls = recordMutations();
    server.use(http.patch("/v1/zones/:id/rrsets", noContent));
    const { user } = await openZone();
    await user.click(screen.getByRole("button", { name: "Edit @ SOA" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit SOA records" });
    expect(within(dialog).getByRole("textbox", { name: "Primary name server" })).toHaveValue("ns1");
    const contact = within(dialog).getByRole("textbox", { name: "Contact email" });
    expect(contact).toHaveValue("hostmaster@example.com");
    const refresh = within(dialog).getByRole("textbox", { name: "Refresh" });
    expect(refresh).toHaveValue("10800");
    await user.clear(refresh);
    await user.type(refresh, "72OO0");
    expect(refresh).toHaveValue("720");
    await user.clear(contact);
    await user.type(contact, "dns-admin@example.net");
    await user.click(within(dialog).getByRole("button", { name: "Save to pending changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Review and apply" }));
    const review = await screen.findByRole("dialog", { name: "Review pending changes" });
    await user.click(within(review).getByRole("button", { name: "Apply 1 change" }));
    await waitFor(() =>
      expect(calls[0]?.body).toMatchObject({
        rrsets: [
          {
            type: "SOA",
            records: [
              { content: "ns1.example.com. dns-admin.example.net. 2026111901 720 3600 604800 3600", disabled: false },
            ],
          },
        ],
      }),
    );
  });

  it("rejects a CNAME next to other records and merges values into an existing RRset", async () => {
    const { user } = await openZone();
    await user.click(screen.getByRole("button", { name: "Add record" }));
    let dialog = await screen.findByRole("dialog", { name: "Add records" });
    await user.type(within(dialog).getByLabelText("Name"), "www");
    await user.click(within(dialog).getByRole("button", { name: /Type/ }));
    await user.click(await screen.findByRole("option", { name: "CNAME" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Target" }), "target.example.net.");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    expect(await within(dialog).findByText("A CNAME cannot share a name with other records.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await user.click(screen.getByRole("button", { name: "Add record" }));
    dialog = await screen.findByRole("dialog", { name: "Add records" });
    await user.type(within(dialog).getByLabelText("Name"), "www");
    await user.type(within(dialog).getByRole("textbox", { name: "IPv4 address" }), "192.0.2.11");
    await user.click(within(dialog).getByRole("button", { name: "Add to pending changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const www = row("www");
    expect(within(www).getByText("192.0.2.10")).toBeInTheDocument();
    expect(within(www).getByText("192.0.2.11")).toBeInTheDocument();
    expect(within(www).getByText("Changed")).toBeInTheDocument();
  });

  it("undoes and discards changes, and never deletes the SOA", async () => {
    const { user } = await openZone();
    const soaRow = within(screen.getByRole("table", { name: "Records" }))
      .getAllByRole("rowheader", { name: "@" })[0]
      ?.closest("tr") as HTMLElement;
    expect(within(soaRow).getByText("SOA")).toBeInTheDocument();
    expect(within(soaRow).queryByRole("button", { name: /Delete/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete www A" }));
    await user.click(screen.getByRole("button", { name: "Undo change to www A" }));
    expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete old A" }));
    await user.click(screen.getByRole("button", { name: "Discard all" }));
    expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
  });

  it("filters records", async () => {
    const { user } = await openZone();
    await user.type(screen.getByRole("searchbox", { name: "Filter records" }), "192.0.2.99");
    await waitFor(() =>
      expect(within(screen.getByRole("table", { name: "Records" })).getAllByRole("row")).toHaveLength(2),
    );
    await user.clear(screen.getByRole("searchbox", { name: "Filter records" }));
    await user.type(screen.getByRole("searchbox", { name: "Filter records" }), "nothing-here");
    expect(await screen.findByText("No records match the filter")).toBeInTheDocument();
  });

  it("keeps changes and shows the error when the PATCH fails", async () => {
    server.use(http.patch("/v1/zones/:id/rrsets", () => apiError(422, "VALIDATION_ERROR", "Record type not allowed")));
    const { user } = await openZone();
    await user.click(screen.getByRole("button", { name: "Delete old A" }));
    await user.click(screen.getByRole("button", { name: "Review and apply" }));
    const review = await screen.findByRole("dialog", { name: "Review pending changes" });
    await user.click(within(review).getByRole("button", { name: "Apply 1 change" }));
    expect(await within(review).findByRole("alert")).toHaveTextContent("Record type not allowed");
    await user.click(within(review).getByRole("button", { name: "Keep editing" }));
    expect(screen.getByRole("region", { name: "Pending changes" })).toHaveTextContent("1 pending change");
  });

  it("offers exactly the types the backend allows in the zone, for any user", async () => {
    const { user } = renderApp("/zones/2.0.192.in-addr.arpa.", { me: userMe });
    await findHeading("2.0.192.in-addr.arpa.");
    expect(await screen.findByText("This zone has no records")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add record" }));
    const dialog = await screen.findByRole("dialog", { name: "Add records" });
    await user.click(within(dialog).getByRole("button", { name: /Type/ }));
    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).toEqual(["PTR", "NS", "TXT"]);
  });

  it("disables adding when no types are allowed", async () => {
    server.use(http.get("/v1/zones/example.com.", () => json({ ...exampleZone, allowed_record_types: [] })));
    await openZone();
    expect(screen.getByRole("button", { name: "Add record" })).toBeDisabled();
    expect(screen.getByText("No record types are allowed in this zone.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit www A" })).toBeEnabled();
  });
});
