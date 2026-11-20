# v1.0.1 — Field validation / Проверка на оборудовании

**Release gate:** PR #1 has been merged. Before publishing v1.0.1, run the checks below on an isolated test bench. PR #2 adds deterministic TCP loopback CI tests; these do not prove physical RS-485 or production-network safety.

**Условие выпуска:** PR #1 уже слит; перед публикацией v1.0.1 необходимо пройти испытания на стенде. PR #2 добавляет TCP-симулятор для CI; зелёный CI не заменяет проверки реальных устройств.

**QA test installers / Установщики для проверки:** [Windows QA (EN)](WINDOWS_QA.md) / [Windows QA (RU)](WINDOWS_QA.ru.md). QA binaries are not an official release.

## Automated protocol coverage / Автоматическое покрытие

The `scanner::field_tests` module runs on 127.0.0.1 with random ephemeral TCP ports.
It verifies the full path from ScanTarget to the real scanner engine, actual socket,
MBAP framing, response classification, register map, and scan completion.

| Automated scenario | Expected outcome |
|---|---|
| FC01/02/03/04 discovery | All outbound frames are reads; no FC05/06/0F/10 |
| FC03 map read + fragmented TCP reply | Correct registers, no extra-byte timeout |
| Modbus exception 02 | Identifies Modbus endpoint, records exception |
| Wrong Transaction ID | No false device discovery |
| Malformed FC03 byte count, wrong response FC | No device discovered; no successful read count |
| Wrong-function/oversized Modbus exception | Rejected, no false device discovery |
| Register map returns fewer words than requested | No values persisted, no successful read count |
| Malformed first adaptive block | Stop splitting; no additional probe cascade |
| Short FC01/FC02 payload (including crafted counts) | Error, never panic or empty success |
| Connected but silent target | Timeout, no false device discovery |
| Invalid FC list (e.g. 03 + 06) | Rejected before opening a socket |

Run locally: `cd src-tauri && cargo test --lib scanner::field_tests -- --nocapture`.
GitHub CI executes it on Windows, Linux and macOS. Tests contain no physical
serial connection and never send packets outside the local loopback interface.

**Physical negative tests for PR #5:** On an isolated Modbus RTU/ASCII bench or
simulator, feed truncated FC01/FC02/FC03 replies, incorrect byte counts and a
wrong-function exception; check that the UI reports an error and does not create
fictitious devices/registers. Keep all tests read-only. Never inject malformed
traffic into production OT networks.

**Not automated:** RS-485 signal integrity, USB adapter behavior, baud/parity
compatibility, real firmware error handling, field safety acceptance.

## Test environment / Стенд

- Windows 10/11 x64, build of the approved QA commit from [WINDOWS_QA.md](WINDOWS_QA.md), USB–RS485 adapter, isolated/test Modbus RTU slave and a Modbus TCP simulator or test device. Match the SHA256 against the CI manifest.
- Записать модель, firmware, COM/driver, тип RS-485, адрес, скорость, биты/чётность/стоп-биты и карту доступных регистров.
- Avoid active network scans of production subnets; for operational OT networks use explicit authorization, 1–8 IP concurrency, and conservative timeouts.
- Do not use write tests on process equipment; use a simulator or disconnected test slave only.

## Lifecycle regression / Регрессия жизненного цикла

PR #4 adds `scanner::lifecycle_tests` to verify that repeated Start is rejected
while Running/Paused, Stop interrupts a silent TCP request, pending register reads
are discarded, and a new scan may start after cleanup.

For serial hardware, explicitly verify a Stop during a long RTU timeout and a
Pause → Stop transition: after the application reports `Stopped`, no new
transaction should be initiated by the scanner. Bytes that were already sent
cannot be canceled.

## Verification matrix / Матрица испытаний

| ID | Procedure / Действие | Acceptance / Критерий |
|---|---|---|
| TCP-01 | FC03 read known register from TCP simulator; repeat 20 times | Correct MBAP/PDU length; no phantom extra-byte timeout; correct data |
| TCP-02 | FC04, Modbus exception 0x02, and invalid transaction ID in mock tests | Read success and correctly classified exception / rejection |
| RTU-01 | FC03 at 1200, 2400, 9600 and 19200 baud on the bench | Valid CRC, full response, no false timeout from an initial 5 ms wait |
| RTU-02 | Delay first response byte > 5 ms but < configured timeout; repeat at 3 baud rates | Response accepted within configured timeout |
| RTU-03 | Invalid CRC and wrong unit ID injected on simulator | Frame rejected and logged; no device false-positive |
| ASCII-01 | Send valid ASCII+LRC response in two chunks with a short gap | Wait until CRLF, then decode and validate |
| SCAN-01 | Scan a known serial unit and a single TCP device | Device found, correct device key and register map |
| SCAN-02 | Try CIDR /0, /8 and /15 in test UI; do not actually scan large subnets | Rejected before memory allocation; /24 accepted |
| SCAN-03 | Map address 65535 with quantity 1; reject quantity 2 from 65535 | No crash/overflow and clear error for out-of-range request |
| SAFE-01 | Try manual write and raw send without opt-in via IPC/test harness | Backend rejects before opening a transport |
| SAFE-02 | Try broadcast write unit 0 and full-ADU raw broadcast on simulator | Rejected; no outgoing write frame |
| SAFE-03 | Confirm a benign write on disconnected simulator only | Modal confirmation, one request, expected simulator response |
| UI-01 | Verify RU/EN labels, exported logs, serial probe RTT | No missing labels; RTT non-zero when a response arrives |
| LIFE-01 | Stop a running RTU scan while awaiting slave response | Stopped promptly without waiting for full timeout; no newly scheduled TX |
| LIFE-02 | Pause the scanner, attempt a second Start, then Stop | Second Start rejected; Stopped after cleanup; restart succeeds |
| LIFE-03 | Stop with pending register reads behind a discovered device | No additional register read after cancellation; pending queue is discarded |

## Evidence / Отчёт

For each row record: **PASS/FAIL/N/A**, timestamp, app/firmware versions,
test target, relevant TX/RX hex, expected/actual behavior, and screenshots/log.
Do not include actual production passwords or sensitive network diagrams.

По каждому пункту фиксировать **PASS/FAIL/N/A**, дату, версии ПО и firmware,
параметры стенда, TX/RX в HEX, ожидаемый/фактический результат и ссылку на лог.
Запрещено добавлять секреты и сведения о действующей сети в публичный GitHub.

### Go / No-go

- **Go:** all CI checks green; no P0/P1 regressions; TCP/RTU/ASCII on the test bench verified; safety tests pass; reviewer approves.
- **No-go:** any unexpected write, incorrect MBAP parsing, RTU timeout on valid frame, arithmetic overflow, or false device detection.

**Do not create a v1.0.1 release tag before the Go criteria are met.**
