import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import { templates, userMe } from "../test/fixtures";
import { json, noContent } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { recordMutations, server } from "../test/server";

describe("templates", () => {
  it("creates, edits, and deletes templates", async () => {
    const calls = recordMutations();
    server.use(
      http.post("/v1/templates", () => json({ ...templates[0], id: 2 }, 201)),
      http.put("/v1/templates/:id", () => json(templates[0])),
      http.delete("/v1/templates/:id", noContent),
    );
    const { user } = renderApp("/templates");
    await findHeading("Templates");
    const table = await screen.findByRole("table", { name: "Templates" });
    expect(within(table).getByText("@ A 192.0.2.1, www CNAME example.com.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Create template" }));
    let dialog = await screen.findByRole("dialog", { name: "Create template" });
    await user.click(within(dialog).getByRole("button", { name: "Save template" }));
    expect(await within(dialog).findByText("Enter a name.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Name"), "Mail");
    await user.click(within(dialog).getByRole("button", { name: "Add record" }));
    const record = within(dialog).getByRole("group", { name: "Record 1" });
    await user.click(within(record).getByRole("button", { name: /Type/ }));
    await user.click(await screen.findByRole("option", { name: "MX" }));
    await user.type(within(record).getByRole("textbox", { name: "Priority" }), "10");
    await user.type(within(record).getByRole("textbox", { name: "Mail server" }), "mail");
    expect(within(record).getByText(".<zone>")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Save template" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Edit Basic web" }));
    dialog = await screen.findByRole("dialog", { name: "Edit Basic web" });
    await user.click(within(dialog).getByRole("button", { name: "Remove record 2" }));
    const content = within(within(dialog).getByRole("group", { name: "Record 1" })).getByRole("textbox", {
      name: "IPv4 address",
    });
    await user.clear(content);
    await user.type(content, "999.0.0.1");
    await user.click(within(dialog).getByRole("button", { name: "Save template" }));
    expect(await within(dialog).findByText(/Enter an IPv4 address/)).toBeInTheDocument();
    await user.clear(content);
    await user.type(content, "192.0.2.5");
    await user.click(within(dialog).getByRole("button", { name: "Save template" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Delete Basic web" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete template" });
    await user.click(within(confirm).getByRole("button", { name: "Delete template" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    await waitFor(() =>
      expect(calls.map((c) => [c.method, c.path, c.body])).toEqual([
        [
          "POST",
          "/v1/templates",
          {
            name: "Mail",
            description: "",
            records: [{ name: "@", type: "MX", ttl: 3600, content: "10 mail" }],
          },
        ],
        [
          "PUT",
          "/v1/templates/1",
          {
            name: "Basic web",
            description: "Apex and www",
            records: [{ name: "@", type: "A", ttl: 3600, content: "192.0.2.5" }],
          },
        ],
        ["DELETE", "/v1/templates/1", undefined],
      ]),
    );
  });

  it("is read only without manage_templates", async () => {
    renderApp("/templates", { me: userMe });
    await screen.findByRole("table", { name: "Templates" });
    expect(screen.queryByRole("button", { name: "Create template" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit Basic web" })).not.toBeInTheDocument();
  });
});
