# Profile Schema v2 / Профили оборудования

Target R06/R14/R21/R22. Existing CSV/JSON remains importable. Schema version is independent of app version.

## Required concepts
Profile identity, vendor, model, optional firmware applicability/range (vendor-specific comparison, not assumed semver), revision, source and validation status. Registers contain table/read FC, zero-based address, width, label, type, order, scale, unit, description, access metadata, optional enum/bitfield definitions and evidence reference. Profile access metadata does not grant write authorization.

## Lifecycle
Import CSV/JSON/XLSX/ODS → preview sheet/column mapping, address convention and types → validation report (conflicts, bad widths, unknown types, invalid scaling) → editable draft → reviewed profile → apply to device with preview. Do not infer undocumented meaning or force profile matches. FC43 can suggest candidates; missing identity remains unknown; manual selection possible. Distinguish source-documented, observed and bench-verified registers.

## Compatibility
Versioned read/write, preserve original legacy file; no lossy downgrade. Stable profile/tag IDs and explicit merge/replace selection; keep user changes unless confirmed. Validate boundary 0..65535 and request limits, bitfield overlap policy, enum base range and source provenance. Include representative fixtures and round-trip tests. No redistribution of proprietary vendor register maps without permission.

See [DATA_MODEL](DATA_MODEL.md), [DATA_TYPES](DATA_TYPES.md), [BACKLOG](BACKLOG.md).
