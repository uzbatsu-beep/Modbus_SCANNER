# Workbench UX / Интерфейс инженерной среды

Approved direction 2026-11-26. Variant B and first shell approved; see [WORKBENCH_SHELL](WORKBENCH_SHELL.md) for the PR #16 implementation boundary. Diagram below is the long-term target, not a claim that all functions are implemented.

```text
Toolbar: project | discovery | registers | monitor | diagnostics | simulator
┌────────────────┬───────────────────────────┬────────────────┐
│ Project tree   │ Tabbed work area          │ Inspector      │
│ Connections    │ Tags / live values        │ Register table │
│ Devices        │ Multi-channel trends      │ Type / scale   │
│ Profiles       │ Map / comparison / tests  │ Quality/source │
├────────────────┴───────────────────────────┴────────────────┤
│ Dockable traffic / diagnostics / test results                │
└──────────────────────────────────────────────────────────────┘
Status: active sessions, transport, actual period, RTT, errors
```

## Interaction requirements
- Resizable/collapsible left tree, inspector and bottom diagnostics. Preserve current workflows during incremental migration; no all-at-once UI rewrite.
- Project → connections → devices → registers/tags; keep stable selection when updates arrive. Independently start/stop sessions; show busy/conflict state and active connection context.
- Editable single/group/adaptive polling, requested period, actual interval and timeout. Changes affecting an active session need explicit apply semantics.
- Table: FC/address space, HEX/DEC, raw, type, engineering value, unit, quality, timestamp. Search/filter/sort, multi-select, copy/export, keyboard navigation. Virtualize large tables if measured necessary.
- Multi-channel trends: independent unit axes, signed range, zero line, zoom/pan, cursor/time marker, freeze view without silently stopping acquisition, quality gaps, actual timestamps, export. Do not combine incompatible units on an unlabeled axis.
- Profile editor: mapping preview, source and validation status, draft/save, unknown enum values, bit inspector.
- Diagnostics: actual RTT, timeout, exception, invalid response, request count, per-device cycle, TX/RX context. Distinguish idle, connecting, running, stopping, fault and stale.
- RU/EN labels, readable themes, accessible status text in addition to color. Existing theme choice retained. No animation or dashboard density that obscures data.

## Acceptance
Validate on desktop at 1024x640 and larger, keyboard flow, multiple devices, slow/faulting endpoint, negative trend, large tag list, resize/docking and persistence. Collect operator feedback on actual isolated bench. See [POLLING_ENGINE](POLLING_ENGINE.md).
