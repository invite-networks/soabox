/**
 * Small typed client for the SOAbox UI API.
 *
 * Every request is same-origin with the session cookie. State-changing requests
 * send the CSRF token cached from the last `Me` response. Errors follow the
 * `{detail, code, status}` shape and surface as {@link ApiError}.
 *
 * The surface mirrors openapi-fetch closely enough that the hooks can move to a
 * generated client once the backend publishes openapi.json.
 */

import type { ErrorBody } from "./types";

/** An error response from the SOAbox API. */
export class ApiError extends Error {
  override readonly name = "ApiError";
  readonly code: string;
  readonly status: number;

  constructor(detail: string, code: string, status: number) {
    super(detail);
    this.code = code;
    this.status = status;
  }

  /** The user-facing message. */
  get detail(): string {
    return this.message;
  }
}

/** Returns true when the error is an {@link ApiError} with the given status. */
export function isApiError(error: unknown, status?: number): error is ApiError {
  return error instanceof ApiError && (status === undefined || error.status === status);
}

/** Returns a message that is safe to show to the user for any error. */
export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    return error.detail;
  }
  return "Something went wrong. Please try again.";
}

let csrfToken: string | null = null;

/** Caches the CSRF token from the current session. */
export function setCsrfToken(token: string | null): void {
  csrfToken = token;
}

/** Returns the cached CSRF token, if any. */
export function getCsrfToken(): string | null {
  return csrfToken;
}

const UNSAFE_METHODS = new Set(["POST", "PUT", "PATCH", "DELETE"]);
const REQUEST_TIMEOUT_MS = 30_000;

type QueryValue = string | number | boolean | null | undefined;

export interface RequestOptions {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  body?: unknown;
  query?: Record<string, QueryValue>;
  signal?: AbortSignal;
}

function isErrorBody(value: unknown): value is ErrorBody {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const v = value as Record<string, unknown>;
  return typeof v["detail"] === "string" && typeof v["code"] === "string";
}

/** Builds a query string, skipping empty values. */
export function buildQuery(query: Record<string, QueryValue> | undefined): string {
  if (!query) {
    return "";
  }
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") {
      continue;
    }
    params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `?${text}` : "";
}

/** Encodes a PowerDNS zone id for use in a path. */
export function zonePath(zoneId: string): string {
  return `/v1/zones/${encodeURIComponent(zoneId)}`;
}

/**
 * Sends a request and parses the JSON response.
 *
 * @returns The parsed body, or `undefined` for `204 No Content`.
 * @throws {ApiError} for any non-2xx response or network failure.
 */
export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const method = options.method ?? "GET";
  const headers = new Headers({ Accept: "application/json" });
  if (options.body !== undefined) {
    headers.set("Content-Type", "application/json");
  }
  if (UNSAFE_METHODS.has(method) && csrfToken) {
    headers.set("X-CSRF-Token", csrfToken);
  }

  const timeout = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
  const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;

  let response: Response;
  try {
    response = await fetch(`${path}${buildQuery(options.query)}`, {
      method,
      headers,
      credentials: "same-origin",
      body: options.body === undefined ? null : JSON.stringify(options.body),
      signal,
    });
  } catch (cause) {
    if (options.signal?.aborted) {
      throw cause;
    }
    throw new ApiError("The server could not be reached.", "NETWORK_ERROR", 0);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  let data: unknown;
  try {
    data = text ? JSON.parse(text) : undefined;
  } catch {
    data = undefined;
  }

  if (!response.ok) {
    if (isErrorBody(data)) {
      throw new ApiError(data.detail, data.code, response.status);
    }
    throw new ApiError(`Request failed with status ${response.status}.`, "HTTP_ERROR", response.status);
  }

  // The response shape is defined by the API contract in docs/spec/v1-api.md.
  return data as T;
}

/** Convenience wrappers. */
export const api = {
  get: <T>(path: string, query?: Record<string, QueryValue>, signal?: AbortSignal) =>
    request<T>(path, { ...(query ? { query } : {}), ...(signal ? { signal } : {}) }),
  post: <T>(path: string, body?: unknown) => request<T>(path, { method: "POST", body }),
  put: <T>(path: string, body?: unknown) => request<T>(path, { method: "PUT", body }),
  patch: <T>(path: string, body?: unknown) => request<T>(path, { method: "PATCH", body }),
  delete: <T>(path: string, body?: unknown) => request<T>(path, { method: "DELETE", body }),
};
