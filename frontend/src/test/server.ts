import { setupServer } from "msw/node";
import { handlers } from "./handlers";

export const server = setupServer(...handlers);

export interface RecordedRequest {
  method: string;
  path: string;
  body: unknown;
  csrf: string | null;
}

/** Records every non-GET request so tests can assert on bodies. */
export function recordMutations(): RecordedRequest[] {
  const calls: RecordedRequest[] = [];
  server.events.on("request:start", ({ request }) => {
    if (request.method === "GET") {
      return;
    }
    const entry: RecordedRequest = {
      method: request.method,
      path: decodeURIComponent(new URL(request.url).pathname),
      body: undefined,
      csrf: request.headers.get("X-CSRF-Token"),
    };
    calls.push(entry);
    void request
      .clone()
      .text()
      .then((text) => {
        entry.body = text ? JSON.parse(text) : undefined;
      });
  });
  return calls;
}
