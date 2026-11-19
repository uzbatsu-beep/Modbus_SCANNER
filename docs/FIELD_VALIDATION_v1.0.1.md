# v1.0.1 — Field validation / Проверка на оборудовании

**Release gate:** This checklist must be executed on a test bench before merging PR #1 and publishing v1.0.1. GitHub Actions verify source/test consistency but do not prove physical RS-485 or production-network safety.

**Условие выпуска:** перед слиянием PR #1 и публикацией v1.0.1 пройти испытания на стенде. Зелёный CI не заменяет проверки реальных устройств.

## Test environment / Стенд

- Windows 10/11 x64, build of PR #1, USB–RS485 adapter, isolated/test Modbus RTU slave and a Modbus TCP simulator or test device.
- Записать модель, firmware, COM/driver, тип RS-485, адрес, скорость, биты/чётность/стоп-биты и карту доступных регистров.
- Avoid active network scans of production subnets; for operational OT networks use explicit authorization, 1–8 IP concurrency, and conservative timeouts.
- Do not use write tests on process equipment; use a simulator or disconnected test slave only.

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
