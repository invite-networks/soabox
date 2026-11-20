# Diagrams

Each diagram is a Markdown file with a fenced Mermaid block. The fenced block is the editable source, and GitHub renders it in place, so there are no separate image exports to keep in sync.

| Diagram | View | Linked from |
|---|---|---|
| [System context](context.md) | Context | Design 1 |
| [Actors and use cases](use-cases.md) | Use cases | Requirements 3 |
| [Components](components.md) | Components | Design 2 |
| [Passthrough write](sequence-passthrough-write.md) | Sequence | FR-KEY-01, FR-AUD-01 |
| [OIDC sign-in](sequence-oidc-sign-in.md) | Sequence | FR-AUTH-03, FR-AUTH-04 |
| [Trust boundaries](trust-boundaries.md) | Data flow | Design 7 |
| [Deployment](deployment.md) | Deployment | Design 9 |
| [Data model](data-model.md) | Entity relationships | Design 4 |
| [API key lifecycle](api-key-lifecycle.md) | State | FR-KEY-07 |

Not drawn: a class diagram. The Rust code is organized by module rather than class hierarchy, and the component and data model views cover the relationships that matter.

Maintenance rules: update a diagram in the same pull request as the change it describes. Quote labels that contain brackets or special characters and use `<br>` for line breaks.
