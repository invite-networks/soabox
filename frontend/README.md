# SOAbox Frontend

The SOAbox web UI. A React single-page app built with Vite and served as static files by the Rust backend. There is no Node runtime in production.

See [ADR-0002](../docs/adr/0002-react-spa-frontend.md) for the architecture and [ADR-0010](../docs/adr/0010-generic-ui-theme.md) for the theme. The API contract is [docs/spec/v1-api.md](../docs/spec/v1-api.md).

## Requirements

- Node 22 (see `.nvmrc`)
- pnpm, version pinned in `package.json` under `packageManager`

`pnpm-workspace.yaml` sets `minimumReleaseAge` so new dependency versions are only installed after they have been published for 72 hours.

## Scripts

| Command                          | What it does                                        |
| -------------------------------- | --------------------------------------------------- |
| `pnpm install --frozen-lockfile` | Install the locked dependencies.                    |
| `pnpm dev`                       | Start the Vite dev server with the backend proxy.   |
| `pnpm build`                     | Build static files into `dist/`.                    |
| `pnpm typecheck`                 | Regenerate the route tree and run `tsc --noEmit`.   |
| `pnpm lint`                      | Biome lint and format check. Warnings fail.         |
| `pnpm format`                    | Apply Biome fixes and formatting.                   |
| `pnpm test`                      | Run the Vitest suite once.                          |
| `pnpm test:coverage`             | Run Vitest with coverage. Fails below 80% of lines. |
| `pnpm test:e2e`                  | Run Playwright against a running SOAbox.            |

## Development

```sh
pnpm install --frozen-lockfile
pnpm dev
```

The dev server proxies `/api`, `/v1`, and `/auth` to the backend, so the browser sees one origin and no CORS setup is needed. The target defaults to `http://localhost:8080`. Point it elsewhere with:

```sh
SOABOX_DEV_BACKEND=http://localhost:9000 pnpm dev
```

`pnpm build` writes `dist/`. The backend serves it with an `index.html` fallback, so deep links such as `/zones/example.com.` load directly.

## Theme

All colors live as CSS custom properties in one file, [`src/styles/theme.css`](src/styles/theme.css). Light values are on `:root`. Dark values apply when the system prefers dark, or when the user picks dark from the user menu, which sets `data-theme` on `<html>` and remembers the choice in `localStorage`.

To restyle SOAbox, change the values in that file. Do not add colors anywhere else. `src/styles/app.css` maps the tokens to Tailwind utilities such as `bg-surface`, `text-fg-muted`, and `bg-accent`.

The theme uses a slate gray scale with one blue accent for primary actions, focus, and the current navigation item. Status colors (green, amber, red) always appear with text or an icon. Fonts are bundled from npm: Inter for the interface and JetBrains Mono for record content, serials, and keys. Nothing loads from a third-party host.

## Layout

```
src/
  api/            types.ts, client.ts, query-client.ts, hooks/ (TanStack Query per resource)
  components/
    ui/           the only place React Aria Components are imported
    zone/         zone tabs and the record editor
  lib/            DNS validation, pending changes, diff parsing, theme, formatting
  routes/         file-based routes (TanStack Router)
  routeTree.gen.ts  generated, committed
  styles/         theme.css and app.css
  test/           MSW handlers, fixtures, render helper
  __tests__/      page tests
e2e/              Playwright specs
```

Application code imports components only from `src/components/ui/`. Those wrappers apply the theme classes and do not accept `className` or `style`.

## API client

`src/api/types.ts` is written by hand from the API contract until the backend publishes `openapi.json`. `src/api/client.ts` sends same-origin requests with the session cookie and adds `X-CSRF-Token` from the cached `/v1/me` response on `POST`, `PUT`, `PATCH`, and `DELETE`. Errors in the `{detail, code, status}` shape become `ApiError`. Components never call `fetch` directly. They use the hooks in `src/api/hooks/`.

A `401` from any query or mutation sends the user to `/login` and returns them afterwards.

## Tests

Vitest with Testing Library and MSW. Handlers in `src/test/handlers.ts` return fixtures typed from `src/api/types.ts`. Unhandled requests fail the test. Page tests render the real router at a path with `renderApp()`.

### End-to-end

Playwright runs against a real SOAbox serving the built UI, not the Vite dev server. Each page gets an axe scan in light and dark mode, and serious or critical violations fail.

```sh
pnpm exec playwright install chromium
SOABOX_E2E_BASE_URL=http://localhost:8080 \
SOABOX_E2E_USERNAME=admin \
SOABOX_E2E_PASSWORD='<password>' \
pnpm test:e2e
```

The specs create zones named `e2e-<id>.test.` and `axe-<id>.test.` and delete them when they finish.
