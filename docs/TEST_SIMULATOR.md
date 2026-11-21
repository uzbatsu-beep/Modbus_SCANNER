# Modbus field simulator / CI protocol smoke (PR #2)

## Purpose

Repeatable **offline** verification of the production scanner's Modbus TCP
handling, without relying on an operator, PLC, VFD, or device connected to the
industrial network. Tests use a real TCP listener bound to \`127.0.0.1:0\`
(a random available local port).

**Scope:** all tests execute the same \`ScannerHandle::start\` and protocol
stack used by the GUI, CLI and REST API. This is not a standalone mock of
\`verify_mbap\`; it validates the end-to-end scan.

## Automated matrix

| ID | Fixture | Expected behavior |
|---|---|---|
| SIM-01 | FC01/02/03/04 read replies | Four read-only ADUs; one discovered device |
| SIM-02 | FC03 + two-register map, fragmented MBAP reply | Registers \`0x002A\` and \`0x002B\` recovered |
| SIM-03 | FC03 exception 0x02 | Exception counted as proof of a responding Modbus slave |
| SIM-04 | Mismatched MBAP transaction ID | Frame rejected; no false device |
| SIM-05 | TCP accepted but no response | Timeout counted; no false device |
| SIM-06 | Probe list contains FC06 write | Scan rejected before any transport is opened |

All tests set \`identify_devices=false\` and \`retries=0\` to make the
number of requests deterministic. TCP port numbers are allocated by the OS
so parallel CI workers do not collide.

## Run / Запуск

From the repository root:

\`\`\`bash
cd src-tauri
cargo test --lib scanner::field_tests -- --nocapture
\`\`\`

All existing tests:

\`\`\`bash
cd src-tauri
cargo test --lib
\`\`\`

This smoke suite is also executed as a separate named CI step in
\`.github/workflows/ci.yml\` on Windows, Linux and macOS.

## How to interpret failures / Как оценивать ошибки

- **Invalid transaction ID accepted:** P0; false discovery risk.
- **Write function observed on discovery wire:** P0; production safety violation.
- **Valid fragmented response rejected:** P1; reliability regression.
- **Exception treated as missing device:** P1; false negative on unsupported register.
- **PDU decoder returns an empty map for valid FC03/04:** P0; read-response direction must be preserved. Discovered by SIM-02 and covered by a direct manual-TCP regression test.
- **Timeout misclassified as valid device:** P1; false positive.

Keep diagnostics and raw frame captures with test results when reporting
regressions, but **do not** upload production network credentials or maps.

## Scope limits / Ограничения

The loopback simulator does not validate USB–RS485 drivers, timing on real
buses, LRC/CRC errors caused by electrical noise, or device-specific
register maps. For hardware acceptance, follow
[FIELD_VALIDATION_v1.0.1.md](FIELD_VALIDATION_v1.0.1.md).

## Reproducibility and physical coverage gap (2026-11-21)

Run `cd src-tauri && cargo test --lib scanner::field_tests -- --nocapture` for the deterministic TCP loopback matrix; include the command, OS, revision and test output when investigating regressions. For CLI/GUI changes, also run lifecycle and protocol tests and Windows QA on the exact candidate SHA. Simulated TCP validates framing and error handling, **not** USB/RS-485 drivers, cable termination, serial inter-frame gaps or real plant safety. Physical release acceptance remains [separate](FIELD_VALIDATION_v1.0.1.md). [Documentation index](INDEX.md).

**RU:** Локальный TCP-симулятор не заменяет RS-485 и реальную работу устройств. Фиксируйте SHA и отчёт теста; стендовый PASS оформляется отдельно.
