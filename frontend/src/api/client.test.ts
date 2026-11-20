import { HttpResponse, http } from "msw";
import { describe, expect, it, vi } from "vitest";
import { apiError, json, noContent } from "../test/handlers";
import { recordMutations, server } from "../test/server";
import { ApiError, api, buildQuery, errorMessage, getCsrfToken, isApiError, setCsrfToken, zonePath } from "./client";

describe("api client", () => {
  it("sends the CSRF token on state-changing requests only", async () => {
    const calls = recordMutations();
    let getHeader: string | null = "unset";
    server.use(
      http.get("/v1/thing", ({ request }) => {
        getHeader = request.headers.get("X-CSRF-Token");
        return json({ ok: true });
      }),
      http.post("/v1/thing", noContent),
    );
    setCsrfToken("tok");
    expect(getCsrfToken()).toBe("tok");
    await expect(api.get("/v1/thing")).resolves.toEqual({ ok: true });
    await expect(api.post("/v1/thing", { a: 1 })).resolves.toBeUndefined();
    expect(getHeader).toBeNull();
    await vi.waitFor(() => expect(calls).toEqual([{ method: "POST", path: "/v1/thing", body: { a: 1 }, csrf: "tok" }]));
  });

  it("parses the error shape", async () => {
    server.use(http.get("/v1/fail", () => apiError(422, "VALIDATION_ERROR", "Bad record")));
    const error = await api.get("/v1/fail").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(isApiError(error, 422)).toBe(true);
    expect((error as ApiError).code).toBe("VALIDATION_ERROR");
    expect(errorMessage(error)).toBe("Bad record");
  });

  it("handles non-JSON errors and network failures", async () => {
    server.use(
      http.get("/v1/html", () => new HttpResponse("<html>", { status: 502 })),
      http.get("/v1/down", () => HttpResponse.error()),
    );
    const html = await api.get("/v1/html").catch((e: unknown) => e);
    expect(isApiError(html, 502)).toBe(true);
    expect((html as ApiError).code).toBe("HTTP_ERROR");
    const down = await api.get("/v1/down").catch((e: unknown) => e);
    expect(isApiError(down, 0)).toBe(true);
    expect(errorMessage(new Error("x"))).toMatch(/Something went wrong/);
  });

  it("builds queries and zone paths", () => {
    expect(buildQuery(undefined)).toBe("");
    expect(buildQuery({ q: "", page: 2, x: undefined, y: null, b: true })).toBe("?page=2&b=true");
    expect(zonePath("0=2F26.2.0.192.in-addr.arpa.")).toBe("/v1/zones/0%3D2F26.2.0.192.in-addr.arpa.");
  });

  it("sends PUT, PATCH, and DELETE", async () => {
    const calls = recordMutations();
    server.use(http.put("/v1/x", noContent), http.patch("/v1/x", noContent), http.delete("/v1/x", noContent));
    await api.put("/v1/x", { a: 1 });
    await api.patch("/v1/x", { b: 2 });
    await api.delete("/v1/x");
    expect(calls.map((c) => c.method)).toEqual(["PUT", "PATCH", "DELETE"]);
  });
});
