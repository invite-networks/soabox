import { screen, waitFor, within } from "@testing-library/react";
import { http } from "msw";
import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "../api/types";
import { historyEntries } from "../test/fixtures";
import { json } from "../test/handlers";
import { findHeading, renderApp } from "../test/render";
import { server } from "../test/server";

function page(start: number, count: number): HistoryEntry[] {
  return Array.from({ length: count }, (_, i) => ({ ...(historyEntries[0] as HistoryEntry), id: start - i }));
}

describe("global history", () => {
  it("filters through the URL and pages with before_id", async () => {
    const seen: URLSearchParams[] = [];
    server.use(
      http.get("/v1/history", ({ request }) => {
        const params = new URL(request.url).searchParams;
        seen.push(params);
        return json({ items: params.get("before_id") ? page(50, 3) : page(100, 50) });
      }),
    );
    const { user, router } = renderApp("/history");
    await findHeading("History");
    await screen.findByRole("table", { name: "History" });
    await user.type(screen.getByLabelText("Zone"), "example.com.");
    await user.type(screen.getByLabelText("Actor"), "admin");
    await user.type(screen.getByLabelText("Operation"), "rrsets.patch");
    await user.type(screen.getByLabelText("From"), "2026-11-01");
    await user.type(screen.getByLabelText("To"), "2026-11-19");
    await user.click(screen.getByRole("button", { name: "Apply filters" }));
    await waitFor(() =>
      expect(router.state.location.search).toEqual({
        zone: "example.com.",
        actor: "admin",
        operation: "rrsets.patch",
        since: "2026-11-01",
        until: "2026-11-19",
      }),
    );
    await waitFor(() => expect(seen.at(-1)?.get("zone")).toBe("example.com."));
    const last = seen.at(-1);
    expect(last?.get("limit")).toBe("50");
    expect(Number(last?.get("until")) - Number(last?.get("since"))).toBeGreaterThan(18 * 86_400);

    await user.click(await screen.findByRole("button", { name: "Load older entries" }));
    await waitFor(() => expect(seen.at(-1)?.get("before_id")).toBe("51"));
    await waitFor(() =>
      expect(within(screen.getByRole("table", { name: "History" })).getAllByRole("row").length).toBe(54),
    );
    expect(screen.queryByRole("button", { name: "Load older entries" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("links zones by their PowerDNS id", async () => {
    renderApp("/history");
    const table = await screen.findByRole("table", { name: "History" });
    expect(within(table).getByRole("link", { name: "0/26.2.0.192.in-addr.arpa." })).toHaveAttribute(
      "href",
      "/zones/0%3D2F26.2.0.192.in-addr.arpa.",
    );
  });

  it("shows a zone name without a link when the zone id is unknown", async () => {
    server.use(
      http.get("/v1/history", () =>
        json({ items: [{ ...(historyEntries[0] as HistoryEntry), zone_id: null, zone_name: "deleted.example." }] }),
      ),
    );
    renderApp("/history");
    const table = await screen.findByRole("table", { name: "History" });
    expect(within(table).getByText("deleted.example.")).toBeInTheDocument();
    expect(within(table).queryByRole("link", { name: "deleted.example." })).not.toBeInTheDocument();
  });

  it("shows an empty state", async () => {
    server.use(http.get("/v1/history", () => json({ items: [] })));
    renderApp("/history?actor=nobody");
    expect(await screen.findByText("No history matches these filters")).toBeInTheDocument();
  });
});
