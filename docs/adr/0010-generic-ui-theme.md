# ADR-0010: Generic, Unbranded UI Theme

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: NFR-UX-01, NFR-A11Y-01

## Context

SOAbox is a public project and must not carry any company's branding.

## Alternatives

- A neutral theme with CSS custom properties.
- An existing company design system.

## Decision

A neutral slate gray scale with one blue accent, semantic status colors paired with text or icons, light and dark mode, self-hosted Inter and JetBrains Mono, and theme tokens in one CSS file. Components are React Aria Components wrapped in a local component layer.

## Rationale

Neutral by default and easy for anyone to restyle. React Aria provides accessible behavior.

## Consequences

No company styling is reused, so the component layer is built for this project.
