# ADR-0001: Rust And Axum Backend

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: All

## Context

SOAbox needs a fast, safe HTTP service that proxies the PowerDNS API and serves a UI. PowerDNS-Admin is a Python Flask application.

## Alternatives

- Rust with Axum: memory safe, low overhead, strong typing, single static binary.
- Python with FastAPI: quick to write, but slower and a larger runtime image.
- Go: good fit, but outside the maintainers' chosen stack.

## Decision

The backend is a Rust 2024 edition crate using Tokio and Axum. It is a library with a `soabox` binary.

## Rationale

A passthrough in the hot path of every DNS change benefits from low and predictable latency (NFR-PERF-01). Rust's type system helps keep authorization rules exhaustive.

## Consequences

Contributors need Rust. Compile times are longer than an interpreted stack.
