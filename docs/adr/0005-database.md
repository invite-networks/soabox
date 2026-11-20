# ADR-0005: SQLite By Default, Postgres Supported, Through SQLx

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: NFR-OPS-07, NFR-OPS-08

## Context

SOAbox stores keys, users, sessions, settings, and history. Small installs want no extra service. Larger installs expect Postgres.

## Alternatives

- SQLite only.
- Postgres only.
- Both, through SQLx with portable queries and per-database migrations.
- Both, through an ORM such as SeaORM.

## Decision

SQLx with the `Any` driver, portable SQL, and separate migration folders for SQLite and Postgres. SQLite in `data/` is the default.

## Rationale

Keeps the dependency set small and the SQL visible. Both databases are exercised in CI.

## Consequences

Queries are not checked at compile time with the `Any` driver, so integration tests on both databases carry that weight. Portable SQL avoids database-specific features.
