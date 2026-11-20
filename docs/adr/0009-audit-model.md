# ADR-0009: Audit By Reading Before And After Each Write

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-AUD-*

## Context

PowerDNS does not report what a change altered, and SOAbox must record who changed what.

## Alternatives

- Read affected RRsets before and after each write and store a diff.
- Store only the request body.
- Fail the client's request when the history write fails.

## Decision

SOAbox reads affected data before and after each write and stores a diff. If the history write fails, the error is logged and the client's response is unchanged.

## Rationale

A diff is what people need when tracing a change. The change already happened in PowerDNS, so failing the response would mislead the client into retrying.

## Consequences

Each audited write costs two extra reads. Direct changes on PowerDNS between the reads would show in the diff, so PowerDNS's API should only be reachable by SOAbox.
