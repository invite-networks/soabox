# ADR-0002: React Single-Page Frontend Served By The Backend

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-UI-*

## Context

The record editor, pending-change review, and admin screens need rich client-side state.

## Alternatives

- React single-page app built to static files.
- Server-rendered templates with htmx: simpler, but awkward for the record editor.
- A Node-based server framework: adds a second runtime in production.

## Decision

A React 19 and TypeScript single-page app built with Vite. The Rust backend serves the built files. No Node runtime in production.

## Rationale

Keeps one production process while giving the editor the interactivity it needs. The OpenAPI spec from the backend types the frontend client.

## Consequences

Two toolchains in development (Cargo and pnpm). The OpenAPI spec must be kept in sync, enforced by a drift check.
