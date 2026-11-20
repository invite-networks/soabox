import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { apiKeys, userMe } from "../test/fixtures";
import { apiError, json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("API keys", () => {
  it("lists keys with hint and status", async () => {
    renderApp("/api-keys");
    await findHeading("API keys");
    const table = await screen.findByRole("table", { name: "API keys" });
    expect(within(table).getByText("...abcd")).toBeInTheDocument();
    expect(within(table).getByText("1 zone, 0 accounts")).toBeInTheDocument();
    expect(within(table).getByText("Disabled")).toBeInTheDocument();
    expect(within(table).getByText("Active")).toBeInTheDocument();
  });

  it("creates a scoped key and shows the secret once", async () => {
    const calls = recordMutations();
    server.use(http.post("/v1/api-keys", () => json({ key: apiKeys[0], secret: "sbx_supersecretvalue" }, 201)));
    const { user } = renderApp("/api-keys");
    await findHeading("API keys");
    await user.click(screen.getByRole("button", { name: "Create API key" }));
    const dialog = await screen.findByRole("dialog", { name: "Create API key" });
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    expect(await within(dialog).findByText("Describe what the key is for.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Description"), "Backup job");
    await user.type(within(dialog).getByLabelText("Zones"), "example.com\nexample.org.");
    await user.click(within(dialog).getByRole("checkbox", { name: "Ops" }));
    await user.click(within(dialog).getByRole("button", { name: /Zone creation/ }));
    await user.click(await screen.findByRole("option", { name: "Web" }));
    await user.type(within(dialog).getByLabelText("Allowed networks"), "not a net");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    expect(await within(dialog).findByText(/Enter one address or network per line/)).toBeInTheDocument();
    await user.clear(within(dialog).getByLabelText("Allowed networks"));
    await user.type(within(dialog).getByLabelText("Allowed networks"), "192.0.2.0/24");
    await user.type(within(dialog).getByLabelText("Expires"), "2027-01-31");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));

    const secret = await screen.findByRole("dialog", { name: "Copy your new API key" });
    expect(within(secret).getByText("This key is shown only once")).toBeInTheDocument();
    expect(within(secret).getByLabelText("API key")).toHaveTextContent("sbx_supersecretvalue");
    await user.click(within(secret).getByRole("button", { name: "Copy key" }));
    expect(await navigator.clipboard.readText()).toBe("sbx_supersecretvalue");
    await user.click(within(secret).getByRole("button", { name: "I have copied the key" }));
    await waitFor(() => expect(screen.queryByText("sbx_supersecretvalue")).not.toBeInTheDocument());

    const body = calls[0]?.body as Record<string, unknown>;
    expect(body).toMatchObject({
      description: "Backup job",
      role: "scoped",
      zones: ["example.com.", "example.org."],
      account_ids: [1],
      create_account_id: 2,
      networks: ["192.0.2.0/24"],
    });
    expect(typeof body["expires_at"]).toBe("number");
  });

  it("offers roles only to admins", async () => {
    const { user, unmount } = renderApp("/api-keys");
    await findHeading("API keys");
    await user.click(screen.getByRole("button", { name: "Create API key" }));
    let dialog = await screen.findByRole("dialog", { name: "Create API key" });
    await user.click(within(dialog).getByRole("button", { name: /Role/ }));
    await user.click(await screen.findByRole("option", { name: "Operator" }));
    expect(within(dialog).queryByLabelText("Zones")).not.toBeInTheDocument();
    unmount();

    const second = renderApp("/api-keys", { me: userMe });
    await findHeading("API keys");
    await second.user.click(screen.getByRole("button", { name: "Create API key" }));
    dialog = await screen.findByRole("dialog", { name: "Create API key" });
    expect(within(dialog).queryByRole("button", { name: /Role/ })).not.toBeInTheDocument();
  });

  it("edits, disables, enables, and deletes keys", async () => {
    const calls = recordMutations();
    server.use(
      http.put("/v1/api-keys/:id", () => json(apiKeys[0])),
      http.post("/v1/api-keys/:id/disable", noContent),
      http.post("/v1/api-keys/:id/enable", noContent),
      http.delete("/v1/api-keys/:id", noContent),
    );
    const { user } = renderApp("/api-keys");
    await screen.findByRole("table", { name: "API keys" });
    await user.click(screen.getByRole("button", { name: "Edit key ...abcd" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit API key" });
    expect(within(dialog).getByLabelText("Zones")).toHaveValue("example.com.");
    await user.click(within(dialog).getByRole("button", { name: "Save key" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Disable" }));
    await user.click(screen.getByRole("button", { name: "Enable" }));
    await user.click(screen.getByRole("button", { name: "Delete key ...wxyz" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete API key" });
    await user.click(within(confirm).getByRole("button", { name: "Delete key" }));
    await waitFor(() =>
      expect(calls.map((c) => `${c.method} ${c.path}`)).toEqual([
        "PUT /v1/api-keys/10",
        "POST /v1/api-keys/10/disable",
        "POST /v1/api-keys/11/enable",
        "DELETE /v1/api-keys/11",
      ]),
    );
  });

  it("shows action errors", async () => {
    server.use(http.post("/v1/api-keys/:id/disable", () => apiError(403, "FORBIDDEN", "Not your key")));
    const { user } = renderApp("/api-keys");
    await screen.findByRole("table", { name: "API keys" });
    await user.click(screen.getByRole("button", { name: "Disable" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Not your key");
  });
});
