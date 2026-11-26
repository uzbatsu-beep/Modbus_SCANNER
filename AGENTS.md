# Engineering agent instructions

## Product direction
Follow [VISION](docs/VISION.md), [ROADMAP](docs/ROADMAP.md) and [BACKLOG](docs/BACKLOG.md). Modbus Scanner is a local-first engineering workbench, not a SCADA or enterprise security product. Prototype priorities: correct engineering data, reliable transport, useful features, responsive UX, measurable performance. Advanced security/RBAC/cloud are later work; preserve existing baseline safeguards and never invent bench evidence.

## Development contract
- Keep Rust/Tauri 2 + Vue/TypeScript and the in-house protocol stack. Keep transport/protocol logic in Rust, not Vue. No needless dependencies or major rewrites.
- Work in bounded PRs against current main; record actual SHA and tests. Do not merge or publish without maintainer authorization. Never bypass required checks.
- Discovery and monitoring remain read-only. Keep explicit manual-write confirmation, authorization limits, cancellation and safe bus ownership. Do not test against live plant devices without approval.
- Use one register identity (device, address space, zero-based address); distinguish read FC from write permissions. Preserve raw words and quality; use one decoder for display, trends, logs and export. Never default missing values to zero or silently reinterpret ambiguous legacy tags.
- Separate user-requested period, actual period, RTT and cycle duration; measure transport performance independently for serial and TCP. No invented benchmarks.
- Prototype schemas may change incompatibly by maintainer decision; reject unsupported versions explicitly and do not overwrite old files. Update generated IPC types via `npm run gen:types` and both RU/EN reference docs when behavior changes.
- Add deterministic offline regressions before or with fixes; test Stop, timeout, invalid frames, profile validation, schema rejection, data decoding and resource ownership as applicable.
- Run npm ci, typecheck/build, cargo check/test, generated binding drift and npm run docs:check as applicable. Report tests not executed and physical validation separately.
- Record new architectural choices in [ADR](docs/adr/README.md); distinguish proposals from implemented functionality.

## Windows-first development (maintainer approved after PR #22)
- Routine PR/main CI: Windows native Rust check/tests, bindings and protocol smoke; Windows QA installers; platform-independent frontend and governance checks retained. Native Linux/macOS jobs are intentionally skipped, not tested. Only Frontend, Governance and Windows backend are required by the development Ruleset. Keep native Linux/macOS jobs available by manual dispatch.
- At final validation restore the five required contexts using the existing live Ruleset and main-final.json; run CI manually on exact main SHA to execute all five jobs across Windows/Linux/macOS; a skipped or missing native job cannot satisfy the official release gate. Linux/macOS installer packaging and platform bench acceptance remain final-stage work; do not claim builds until performed. No weakening of PR, tag, bench or Windows QA protection.
