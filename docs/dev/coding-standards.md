# Coding Standards

## Rust

- Edition 2024. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must pass.
- `thiserror` for module error types, `anyhow` only in the binary.
- No `unwrap()` or `expect()` on request paths. Startup code may `expect` with a clear message.
- Async with Tokio. Every outbound call has a timeout.
- Parameterized SQL only.
- Public items have doc comments.

## TypeScript And React

- Strict TypeScript. Biome for lint and format.
- Application code imports UI components only from `src/components/ui/`.
- Server calls go through TanStack Query and the generated `openapi-fetch` client.
- Forms use React Hook Form with Zod.
- No `style` props. No secrets or tokens in browser storage.

## API Rules

- `/v1` errors use `{"detail", "code", "status"}`. `/api` errors use the PowerDNS shape.
- New `/v1` endpoints carry `utoipa` annotations and the committed `openapi.json` is regenerated.

## Git

- Branches: `<type>/<short-description>`, for example `feat/passthrough-core`.
- Conventional Commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.
- Pull requests aim for under 400 changed lines where practical.
- Pre-commit hooks run formatters, linters, and the standard file checks.
