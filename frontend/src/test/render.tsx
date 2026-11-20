import { createMemoryHistory } from "@tanstack/react-router";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import type { Me } from "../api/types";
import { App, createApp } from "../app";
import { json } from "./handlers";
import { server } from "./server";

/** Renders the whole app at a path with a fresh router and query client. */
export function renderApp(path: string, options: { me?: Me } = {}) {
  if (options.me) {
    server.use(http.get("/v1/me", () => json(options.me)));
  }
  const history = createMemoryHistory({ initialEntries: [path] });
  const { router, queryClient } = createApp(history);
  const user = userEvent.setup();
  const utils = render(<App router={router} queryClient={queryClient} />);
  return { ...utils, router, queryClient, user };
}

/** Waits for the page heading. */
export function findHeading(name: string | RegExp) {
  return screen.findByRole("heading", { level: 1, name });
}

/** Searches a zone picker and chooses the matching zone. */
export async function pickZone(user: ReturnType<typeof userEvent.setup>, container: HTMLElement, zone: string) {
  await user.type(within(container).getByRole("combobox", { name: "Zones" }), zone.replace(/\.$/, ""));
  await user.click(await screen.findByRole("option", { name: zone }));
}
