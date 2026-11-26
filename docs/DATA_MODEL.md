# Engineering data model / Модель данных

Target design, 2026-11-26; not current implementation. R02 precedes R01.

## Identity
- Project: stable project ID, schema version, devices, connections, tag definitions, views and session references.
- Connection: stable ID; transport, endpoint, port or serial configuration, framing, timeout. Separate mutable connection settings from device identity.
- Device: stable ID, connection ID, Unit ID, reported identity/firmware and optional profile reference. Preserve legacy target keys as migration metadata, not permanent IDs.
- Register key: (device ID, address space, zero-based PDU address). Address spaces: coils (FC01), discrete inputs (FC02), holding (FC03), input (FC04). Read FC is derived from address space; writes and access rights are separate.
- Tag: stable ID, register key, length, decode spec, scale, unit, label, optional profile origin. Several interpretations of the same raw range are allowed; a watch refers to tag ID, not just address.
- Sample: tag ID, session ID, timestamp, raw words/bits, typed value, quality, transaction reference. Retain last good separately from latest observation; missing/invalid is not zero.

## Migration
Add versioned project/config reader; retain original source on migration. If FC cannot be resolved unambiguously from legacy records, keep unresolved and request explicit user selection before polling. Never choose FC03 merely because it is common. Prevent identity collisions across FC, device, transport and multiword tags. Preserve user-edited tags when applying profiles via a previewed merge/replace operation.

## Quality
Good, stale, timeout, Modbus exception (with code), invalid response, transport failure, decode error, not sampled, unresolved. Track raw acquisition separately from decoding. Use monotonic elapsed measurements and wall-clock timestamps for export.

## Acceptance
Round-trip old/new project fixtures; distinguish same address in FC03/FC04 and two devices; ensure profile application, monitor, chart, log and export retain same tag identity and value. Generated IPC types checked in CI. See [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md).
