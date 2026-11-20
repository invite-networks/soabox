import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { accounts, settings, tsigKeys, users } from "../test/fixtures";
import { apiError, json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

function rowFor(table: HTMLElement, name: string) {
  return within(table).getByRole("rowheader", { name }).closest("tr") as HTMLElement;
}

describe("users", () => {
  it("lists users, approves pending users, and runs row actions", async () => {
    const calls = recordMutations();
    server.use(
      http.put("/v1/users/:id", () => json(users[1])),
      http.post("/v1/users/:id/revoke-sessions", noContent),
      http.delete("/v1/users/:id/totp", noContent),
      http.delete("/v1/users/:id", noContent),
      http.post("/v1/users/:id/password", noContent),
    );
    const { user } = renderApp("/admin/users");
    await findHeading("Users");
    const table = await screen.findByRole("table", { name: "Users" });
    const bob = rowFor(table, "bob");
    expect(within(bob).getByText("Pending approval")).toBeInTheDocument();
    expect(within(bob).getByText("Single sign-on")).toBeInTheDocument();
    await user.click(within(bob).getByRole("button", { name: "Approve" }));

    // Admin row: no delete for yourself, but reset password and two-factor are offered.
    await user.click(within(rowFor(table, "admin")).getByRole("button", { name: "Actions for admin" }));
    const menu = await screen.findByRole("menu", { name: "Actions for admin" });
    expect(within(menu).queryByRole("menuitem", { name: "Delete" })).not.toBeInTheDocument();
    await user.click(within(menu).getByRole("menuitem", { name: "Reset password" }));
    const reset = await screen.findByRole("dialog", { name: "Reset password for admin" });
    await user.type(within(reset).getByLabelText("New password"), "short");
    await user.click(within(reset).getByRole("button", { name: "Reset password" }));
    expect(within(reset).getByText("Use at least 12 characters.")).toBeInTheDocument();
    await user.clear(within(reset).getByLabelText("New password"));
    await user.type(within(reset).getByLabelText("New password"), "a much longer password");
    await user.click(within(reset).getByRole("button", { name: "Reset password" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(within(rowFor(table, "admin")).getByRole("button", { name: "Actions for admin" }));
    await user.click(await screen.findByRole("menuitem", { name: "Reset two-factor" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Reset" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await user.click(within(rowFor(table, "bob")).getByRole("button", { name: "Actions for bob" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke sessions" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke sessions" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await user.click(within(rowFor(table, "bob")).getByRole("button", { name: "Actions for bob" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Delete user" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await waitFor(() =>
      expect(calls.map((c) => [c.method, c.path, c.body])).toEqual([
        [
          "PUT",
          "/v1/users/3",
          { display_name: "Bob", email: "", role: "user", status: "active", zones: ["example.org."], account_ids: [1] },
        ],
        ["POST", "/v1/users/1/password", { password: "a much longer password" }],
        ["DELETE", "/v1/users/1/totp", undefined],
        ["POST", "/v1/users/3/revoke-sessions", undefined],
        ["DELETE", "/v1/users/3", undefined],
      ]),
    );
  });

  it("creates and edits users", async () => {
    const calls = recordMutations();
    server.use(
      http.post("/v1/users", () => json(users[0], 201)),
      http.put("/v1/users/:id", () => json(users[1])),
    );
    const { user } = renderApp("/admin/users");
    await screen.findByRole("table", { name: "Users" });
    await user.click(screen.getByRole("button", { name: "Create local user" }));
    const create = await screen.findByRole("dialog", { name: "Create local user" });
    await user.click(within(create).getByRole("button", { name: "Create user" }));
    expect(await within(create).findByText("Enter a username.")).toBeInTheDocument();
    await user.type(within(create).getByLabelText("Username"), "carol");
    await user.type(within(create).getByLabelText("Display name"), "Carol");
    await user.type(within(create).getByLabelText("Email"), "carol@example.test");
    await user.type(within(create).getByLabelText("Password"), "twelve chars ok");
    await user.click(within(create).getByRole("button", { name: /Role/ }));
    await user.click(await screen.findByRole("option", { name: "Operator" }));
    await user.click(within(create).getByRole("button", { name: "Create user" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const table = screen.getByRole("table", { name: "Users" });
    await user.click(within(rowFor(table, "bob")).getByRole("button", { name: "Actions for bob" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const edit = await screen.findByRole("dialog", { name: "Edit bob" });
    await user.type(within(edit).getByLabelText("Zones"), "\nexample.com");
    await user.click(within(edit).getByRole("checkbox", { name: "Web" }));
    await user.click(within(edit).getByRole("button", { name: /Status/ }));
    await user.click(await screen.findByRole("option", { name: "Disabled" }));
    await user.click(within(edit).getByRole("button", { name: "Save user" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await waitFor(() =>
      expect(calls.map((c) => c.body)).toEqual([
        {
          username: "carol",
          display_name: "Carol",
          email: "carol@example.test",
          role: "operator",
          password: "twelve chars ok",
        },
        {
          display_name: "Bob",
          email: "",
          role: "user",
          status: "disabled",
          zones: ["example.org.", "example.com."],
          account_ids: [1, 2],
        },
      ]),
    );
  });
});

describe("accounts", () => {
  it("creates, edits, and deletes accounts", async () => {
    const calls = recordMutations();
    server.use(
      http.post("/v1/accounts", () => json(accounts[0], 201)),
      http.put("/v1/accounts/:id", () => json(accounts[0])),
      http.delete("/v1/accounts/:id", () => apiError(409, "CONFLICT", "Account still owns zones")),
    );
    const { user } = renderApp("/admin/accounts");
    await findHeading("Accounts");
    const table = await screen.findByRole("table", { name: "Accounts" });
    expect(within(rowFor(table, "Ops")).getAllByText("1")).toHaveLength(2);

    await user.click(screen.getByRole("button", { name: "Create account" }));
    let dialog = await screen.findByRole("dialog", { name: "Create account" });
    await user.click(within(dialog).getByRole("button", { name: "Create account" }));
    expect(within(dialog).getByText("Enter a name.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Name"), "DNS team");
    await user.type(within(dialog).getByLabelText("Description"), "Shared zones");
    await user.click(within(dialog).getByRole("button", { name: "Create account" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Edit Ops" }));
    dialog = await screen.findByRole("dialog", { name: "Edit Ops" });
    await user.type(within(dialog).getByLabelText("Zones"), "\nbad zone");
    await user.click(within(dialog).getByRole("button", { name: "Save account" }));
    expect(within(dialog).getByText("Enter one zone name per line.")).toBeInTheDocument();
    await user.clear(within(dialog).getByLabelText("Zones"));
    await user.type(within(dialog).getByLabelText("Zones"), "example.com.\nexample.org");
    await user.click(await within(dialog).findByRole("checkbox", { name: "Admin" }));
    await user.click(within(dialog).getByRole("button", { name: "Save account" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Delete Web" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete account" });
    await user.click(within(confirm).getByRole("button", { name: "Delete account" }));
    expect(await within(confirm).findByRole("alert")).toHaveTextContent("Account still owns zones");

    expect(calls.slice(0, 2).map((c) => c.body)).toEqual([
      { name: "DNS team", description: "Shared zones" },
      { name: "Ops", description: "Operations team", zones: ["example.com.", "example.org."], user_ids: [2, 1] },
    ]);
  });
});

describe("settings", () => {
  it("edits every setting and saves", async () => {
    const calls = recordMutations();
    server.use(http.put("/v1/settings", async ({ request }) => json(await request.json())));
    const { user } = renderApp("/admin/settings");
    await findHeading("Settings");
    await user.click(await screen.findByRole("switch", { name: "Users can create zones" }));
    await user.click(screen.getByRole("switch", { name: "Automatic PTR records" }));
    await user.click(screen.getByRole("button", { name: /Local sign-in when single sign-on is on/ }));
    await user.click(await screen.findByRole("option", { name: "Deny, except for admins" }));
    await user.click(screen.getByRole("button", { name: /New single sign-on users/ }));
    await user.click(await screen.findByRole("option", { name: "Create as User" }));
    const forward = screen.getByRole("group", { name: "Record types for forward zones" });
    await user.click(within(forward).getByRole("checkbox", { name: "TLSA" }));
    await user.click(within(forward).getByRole("checkbox", { name: "CAA" }));
    const reverse = screen.getByRole("group", { name: "Record types for reverse zones" });
    await user.click(within(reverse).getByRole("checkbox", { name: "CNAME" }));
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Settings saved.")).toBeInTheDocument();
    expect(calls[0]?.body).toEqual({
      ...settings,
      local_login_with_oidc: "deny",
      oidc_new_user: "create",
      user_can_create_zones: true,
      auto_ptr: false,
      record_types_forward: ["A", "AAAA", "CNAME", "MX", "TXT", "SRV", "NS", "TLSA"],
      record_types_reverse: ["PTR", "NS", "TXT", "CNAME"],
    });
  });

  it("shows save errors", async () => {
    server.use(http.put("/v1/settings", () => apiError(422, "VALIDATION_ERROR", "Unknown record type")));
    const { user } = renderApp("/admin/settings");
    await user.click(await screen.findByRole("button", { name: "Save settings" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Unknown record type");
  });
});

describe("TSIG keys", () => {
  it("creates, views, and deletes keys", async () => {
    const calls = recordMutations();
    server.use(
      http.post("/v1/tsigkeys", () => json({ id: "new.", name: "new", algorithm: "hmac-sha512", key: "bmV3" }, 201)),
      http.delete("/v1/tsigkeys/:id", noContent),
    );
    const { user } = renderApp("/admin/tsig");
    await findHeading("TSIG keys");
    await screen.findByRole("table", { name: "TSIG keys" });

    await user.click(screen.getByRole("button", { name: "View transfer" }));
    let view = await screen.findByRole("dialog", { name: "TSIG key" });
    expect(await within(view).findByLabelText("Secret")).toHaveTextContent(btoa("secret-key"));
    await user.click(within(view).getByRole("button", { name: "Close" }));

    await user.click(screen.getByRole("button", { name: "Create TSIG key" }));
    const create = await screen.findByRole("dialog", { name: "Create TSIG key" });
    await user.type(within(create).getByLabelText("Name"), "bad name");
    await user.type(within(create).getByLabelText("Secret"), "***");
    await user.click(within(create).getByRole("button", { name: "Create key" }));
    expect(within(create).getByText(/Enter a key name/)).toBeInTheDocument();
    expect(within(create).getByText(/Enter the secret in base64/)).toBeInTheDocument();
    await user.clear(within(create).getByLabelText("Name"));
    await user.type(within(create).getByLabelText("Name"), "new");
    await user.clear(within(create).getByLabelText("Secret"));
    await user.click(within(create).getByRole("button", { name: /Algorithm/ }));
    await user.click(await screen.findByRole("option", { name: "hmac-sha512" }));
    await user.click(within(create).getByRole("button", { name: "Create key" }));
    view = await screen.findByRole("dialog", { name: "TSIG key" });
    await user.click(within(view).getByRole("button", { name: "Close" }));

    await user.click(screen.getByRole("button", { name: `Delete ${tsigKeys[0]?.name}` }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Delete key" }));
    await waitFor(() =>
      expect(calls.map((c) => [c.method, c.path, c.body])).toEqual([
        ["POST", "/v1/tsigkeys", { name: "new", algorithm: "hmac-sha512" }],
        ["DELETE", "/v1/tsigkeys/transfer.", undefined],
      ]),
    );
  });
});
