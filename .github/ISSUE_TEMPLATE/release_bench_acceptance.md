---
name: "Release bench acceptance / Стендовая приёмка релиза"
about: "Document isolated physical Modbus hardware acceptance for a specific release SHA."
title: "[BENCH] vX.Y.Z — hardware acceptance"
labels: []
assignees: []
---

<!-- Keep field names below EXACTLY as written: the Release gate validates them. -->
<!-- Заполните данные фактического изолированного испытания. -->

Source SHA: REPLACE_WITH_40_CHARACTER_MAIN_SHA
Bench result: PENDING
Hardware: REPLACE_WITH_DEVICE_MODEL_AND_FIRMWARE
Transport: REPLACE_WITH_MODBUS_RTU_ASCII_TCP_SETTINGS
Test summary: REPLACE_WITH_ACTUAL_TEST_RESULTS

## Test evidence / Подтверждения
- Date (UTC) / Дата:
- Engineer / Исполнитель:
- Software version / Версия:
- Physical wiring / Подключение:
- Function codes and scenarios / FC и сценарии:
- Normal response, Modbus exception, timeout, CRC/MBAP mismatch / Поведение:
- Scan Stop, Quiet Mode, IP policy and writes only on an isolated test bench / Проверки безопасности:
- Logs, images, QA run and installer SHA256 / Логи и контрольные суммы:

## Sign-off / Итог
Only after a successful physical test, change `Bench result: PENDING` above to
`Bench result: PASS`. On failure use `Bench result: FAIL`. Do not assert
PASS solely from a simulator, unit tests, CI or QA installers.

Измените результат на `PASS` только после фактических испытаний на стенде.
Если испытания неудачны — укажите `FAIL`. Результаты CI не заменяют стенд.
