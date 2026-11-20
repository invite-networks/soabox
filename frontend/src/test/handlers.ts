import { HttpResponse, http, type JsonBodyType } from "msw";
import type { ErrorBody } from "../api/types";
import * as f from "./fixtures";

/** Responds with JSON typed from src/api/types.ts. */
export function json<T>(data: T, status = 200) {
  return HttpResponse.json(data as unknown as JsonBodyType, { status });
}

export function apiError(status: number, code: string, detail: string) {
  const body: ErrorBody = { detail, code, status };
  return json(body, status);
}

export const noContent = () => new HttpResponse(null, { status: 204 });

/** Default handlers: a signed-in admin with sample data. */
export const handlers = [
  http.get("/auth/config", () => json(f.authConfig)),
  http.post("/auth/logout", noContent),
  http.get("/v1/me", () => json(f.adminMe)),
  http.get("/v1/dashboard", () => json(f.dashboard)),
  http.get("/v1/server", () => json(f.serverInfo)),
  http.get("/v1/server/statistics", () => json(f.statistics)),
  http.get("/v1/server/config", () => json(f.serverConfig)),
  http.get("/v1/zones", ({ request }) => {
    const q = new URL(request.url).searchParams.get("q") ?? "";
    const items = f.zoneSummaries.filter((z) => z.name.includes(q));
    return json({ items, total: items.length });
  }),
  http.get("/v1/zones/:id", ({ params }) => {
    if (params["id"] === f.exampleZone.id) {
      return json(f.exampleZone);
    }
    const summary = f.zoneSummaries.find((z) => z.id === params["id"]);
    if (!summary) {
      return apiError(404, "NOT_FOUND", "Zone not found");
    }
    const reverse = summary.name.endsWith("in-addr.arpa.");
    return json({
      ...summary,
      soa_edit_api: "",
      api_rectify: false,
      nsec3param: "",
      allowed_record_types: reverse ? ["PTR", "NS", "TXT"] : ["A", "AAAA", "CNAME"],
      rrsets: [],
    });
  }),
  http.get("/v1/zones/:id/history", () => json({ items: f.historyEntries })),
  http.get("/v1/zones/:id/cryptokeys", () => json(f.cryptoKeys)),
  http.get("/v1/zones/:id/metadata", () => json(f.metadata)),
  http.get("/v1/history", () => json({ items: f.historyEntries })),
  http.get("/v1/templates", () => json(f.templates)),
  http.get("/v1/accounts", () => json(f.accounts)),
  http.get("/v1/users", () => json(f.users)),
  http.get("/v1/api-keys", () => json(f.apiKeys)),
  http.get("/v1/settings", () => json(f.settings)),
  http.get("/v1/tsigkeys", () => json(f.tsigKeys)),
  http.get("/v1/tsigkeys/:id", () => json({ ...f.tsigKeys[0], key: btoa("secret-key") })),
];
