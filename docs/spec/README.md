# Specifications

The authoritative statement of what SOAbox does and how it is built.

| Document | Contents |
|---|---|
| [Requirements](requirements.md) | Scope, actors, functional and nonfunctional requirements with stable IDs and acceptance criteria. |
| [Design](design.md) | Components, interfaces, data model, security boundaries, deployment, and failure handling. |
| [UI API contract](v1-api.md) | The `/auth` and `/v1` endpoints the browser UI uses. |
| [Test plan](test-plan.md) | How each requirement is verified, and the release gates. |

Maintenance rules:

- Requirement IDs are stable and never reused. A removed requirement is marked withdrawn, not deleted.
- A change to behavior updates the requirement, the design, and the test plan in the same pull request as the code.
- Decisions behind the design are recorded in [ADRs](../adr/README.md).
