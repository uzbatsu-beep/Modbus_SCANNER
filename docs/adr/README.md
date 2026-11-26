# Architecture decisions

Dated decisions, 2026-11-26. Direction approved; detailed contracts proposed for implementation and validation.

| Decision | Rationale | Consequence |
|---|---|---|
| ADR-001 Local-first modular Rust/Tauri workbench | Reuse existing core and desktop skills | No rewrite or service platform in prototype |
| ADR-002 Register identity includes address space | Prevent FC03/FC04 collisions | Versioned migration; unresolved legacy tags are not guessed |
| ADR-003 One canonical decoder and quality model | Consistent table/chart/log/export | Raw words retained, exact integers preserved |
| ADR-004 Central connection ownership, independent sessions | Reliable multi-device engineering | Serialize shared serial/gateway resources; bounded parallel independent endpoints |
| ADR-005 User-selectable polling modes | Device-specific constraints and engineering control | Actual timing measured; adaptive mode bounded |
| ADR-006 Prototype-first priorities | Deliver functionality and correct data before enterprise controls | Preserve baseline safeguards, defer expanded RBAC/security architecture |

Detailed design: [data model](../DATA_MODEL.md), [polling engine](../POLLING_ENGINE.md), [UX](../UX_DESIGN.md), [vision](../VISION.md). Future decisions use separate numbered records with context, alternatives, tradeoffs and tests.
