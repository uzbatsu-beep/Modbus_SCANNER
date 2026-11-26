# Engineering data model / Модель данных

Target design, 2026-11-26; not current implementation. R02 precedes R01.

## Identity
- Project: stable project ID, schema version, devices, connections, tag definitions, views and session references.
- Connection: stable ID; transport, endpoint, port or serial configuration, framing, timeout. Separate mutable connection settings from device identity.
- Device: stable ID, connection ID, Unit ID, reported identity/firmware and optional profile reference. Preserve legacy target keys as migration metadata, not permanent IDs.
- Register key: (device ID, address space, zero-based PDU address). Address spaces: coils (FC01), discrete inputs (FC02), holding (FC03), input (FC04). Read FC is derived from address space; writes and access rights are separate.
- Tag: stable ID, register key, length, decode spec, scale, unit, label, optional profile origin. Several interpretations of the same raw range are allowed; a watch refers to tag ID, not just address.
- Sample: tag ID, session ID, timestamp, raw words/bits, typed value, quality, transaction reference. Retain last good separately from latest observation; missing/invalid is not zero.

## Prototype schema boundary (PR #13)

Maintainer decision: backward compatibility with old projects is not required at prototype stage. Project schema v2 is a clean break. No automatic migration of v1 tags; unsupported versions are rejected before state/file changes. Save to a new file; existing incompatible projects are not overwritten. The config uses `config-v2.json`, leaving the old `config.json` untouched. CSV/JSON profiles with explicit FC01–FC04 retain their existing format.

In PR #13 the current device_key is retained until the connection/device manager is implemented. Implement AddressSpace, RegisterKey, RegisterSpan, independent TagDefinition.id and optional profile_id. Manual tags have independently generated IDs; profile tags use deterministic identity (device, profile, function, address, name). Reapplying a profile replaces only its tags on that device. Unchanged tags retain their ID; changed source/name produces a new ID. Duplicate identical profile identity is rejected. Multiple differently named interpretations of the same source are allowed.

Current scalar numeric types consume one or two registers; bit spaces accept one UINT16 (0/1) until BOOL support. ASCII profile width is one word until the extended type model. Bounds and decode width are validated. FC01–FC04 are the only read functions mapped to address spaces. Write permissions are not inferred.

## Quality
Good, stale, timeout, Modbus exception (with code), invalid response, transport failure, decode error, not sampled, unresolved. Track raw acquisition separately from decoding. Use monotonic elapsed measurements and wall-clock timestamps for export.

## Acceptance
Round-trip new v2 fixtures; reject unsupported versions; distinguish same address in FC03/FC04 and two devices; ensure profile application, monitor, chart, log and export retain same tag identity and value. Generated IPC types checked in CI. See [DATA_TYPES](DATA_TYPES.md), [PROFILE_SCHEMA](PROFILE_SCHEMA.md).

PR #14 implements the first measurement engine: frozen watch, session-scoped observation sequence, raw/typed quality, bounded history, deltas and Logger v2. See [MEASUREMENT_ENGINE](MEASUREMENT_ENGINE.md).
