# Security & operational safety / Безопасность

## Discovery is read-only / Обнаружение только читает

During discovery the scanner sends **only read function codes** (FC01–FC04).
This is intentional: scanning an unknown device must never change its state.
Manual write functions (FC05, FC06, FC10) are available only in the Manual tab.
As of v1.0.1, each write and raw-frame operation requires explicit confirmation
in the UI and a per-request opt-in field verified by Rust. Serial broadcast writes
to unit 0 are blocked. This mitigates accidental writes but does not constitute
authentication or authorization against compromised frontend code.

При обнаружении сканер отправляет **только функции чтения** (FC01–FC04) — сканирование
неизвестного устройства не должно менять его состояние. Функции записи FC05/06/10 доступны только во вкладке Manual после подтверждения;
Rust проверяет флаг согласия для каждого запроса. Broadcast-запись (unit 0)
запрещена. Эти проверки защищают от случайной записи, но не заменяют авторизацию.

## Broadcast / Broadcast

Serial unit id `0` is a broadcast address. Devices do not answer broadcast reads,
so it is excluded from meaningful scans. A **broadcast write** (unit 0) affects **every device on the bus**
and is blocked in the v1.0.1 manual backend. Unit ids 248–255 are reserved by the Modbus specification.

Последовательный адрес `0` — broadcast. Устройства не отвечают на broadcast-чтение,
поэтому он бесполезен при сканировании. **Broadcast-запись** (unit 0) затрагивает **все устройства на шине**;
в v1.0.1 ручной обработчик блокирует такую отправку. Адреса 248–255 зарезервированы спецификацией.

## Gateways / Шлюзы

A Modbus TCP gateway routes by unit id to serial slaves. Probing unit ids 1 and
255 is harmless, but a deep scan with ids 2–247 can reach real devices behind the
gateway. Treat a gateway as a production boundary: coordinate scanning with the
system owner.

Modbus TCP-шлюз маршрутизирует по unit id на последовательные устройства. Зондирование
id 1 и 255 безопасно, но глубокое сканирование с id 2–247 может дойти до реальных
устройств за шлюзом. Считайте шлюз границей производственной системы и согласовывайте
сканирование с её владельцем.

## Active scanning in OT / Активное сканирование в OT

Academic and standards sources ([IEEE 11471618](https://ieeexplore.ieee.org/document/11471618/),
[MOSTO](https://www.sciencedirect.com/science/article/pii/S0167404823002833),
[ISA/IEC 62443](https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards))
note that active scanning can affect industrial networks and that audits must not
interfere with ICS workflows. Therefore:

- discovery stays strictly read-only (FC01–FC04);
- optional **OT Quiet Mode** caps total scanner transmissions at 2 requests/sec
  (including retries, FC43 and register reads); adaptive backoff remains future work;
- passive fingerprinting is preferred over aggressive probing where possible;
- write-access testing is a separate opt-in mode with double confirmation and rollback.

Научные работы и стандарты отмечают, что активное сканирование может влиять на
промышленные сети, а аудит не должен мешать работе ICS. Поэтому обнаружение
остаётся строго read-only (FC01–FC04), «тихий» режим ограничит частоту запросов,
а тесты записи будут вынесены в отдельный режим с подтверждением и rollback.

## Network authorization / Права на сеть

- Only scan networks and hosts you own or are authorized to test.
- IP sweeps generate many connection attempts in a short time; on shared networks
  this can trip IDS/IPS or overload fragile equipment.
- The default concurrency value (32) may still open many sockets in parallel;
  prefer 1–8 on OT/ICS networks and enable Quiet Mode.
- PR #7: empty IP allowlists deny all TCP/UDP scans, and explicit excluded
  addresses always take priority over authorized CIDR scopes.
- The local HTTP/WebSocket API requires an environment Bearer token and
  blocks foreign browser origins. Protect the token from untrusted processes.

- Сканируйте только те сети и хосты, которыми вы владеете или на которые у вас есть
  разрешение.
- IP-перебор генерирует много попыток подключения за короткое время — на общих сетях
  это может сработать с IDS/IPS или перегрузить чувствительное оборудование.
- Значение параллелизма по умолчанию (32) допускает много одновременных
  TCP-подключений; в OT-сетях используйте 1–8 и включайте Quiet Mode.
- PR #7: пустой список разрешённых IP запрещает TCP/UDP-сканирование,
  а явно исключённые адреса имеют приоритет над разрешёнными.
- Локальный REST/WebSocket API требует токен Bearer и блокирует чужие Origin.
  Защищайте переменную окружения с токеном от посторонних процессов.

## Data handling / Работа с данными

- Scan results and exported files may contain process values, device addresses and
  configuration details — treat them as sensitive.
- The app stores configuration in `config.json` under the Tauri app config dir;
  it contains no credentials by design.
- Exported files are written to the path the user chooses; the app never uploads
  data anywhere (no network calls beyond Modbus targets).

- Результаты и экспортированные файлы могут содержать технологические значения,
  адреса устройств и детали конфигурации — обращайтесь с ними как с конфиденциальными.
- Приложение хранит настройки в `config.json` каталога конфигурации Tauri;
  учётных данных в них нет по дизайну.
- Файлы экспорта записываются по пути, выбранному пользователем; приложение никуда
  не отправляет данные (кроме запросов Modbus).

## Tauri surface / Поверхность Tauri

- Capabilities are minimal: `core:default` + `shell:allow-open`.
- The frontend receives raw response hex and values; no arbitrary Rust code can be
  invoked from the WebView beyond registered commands.
- Raw frame sending (`send_raw_frame`) bypasses PDU builders: a malformed frame can
  trigger unexpected behaviour in a device. Use it only for debugging on equipment
  you can safely reset.

- Права минимальны: `core:default` + `shell:allow-open`.
- Фронтенд получает hex ответов и значения; из WebView недоступен произвольный Rust
  код, только зарегистрированные команды.
- Отправка raw-кадра обходит сборщики PDU: некорректный кадр может вызвать неожиданное
  поведение устройства. Используйте её только для отладки на оборудовании, которое
  можно безопасно перезапустить.

## Reporting vulnerabilities / Сообщение об уязвимостях

The project is at MVP stage; no security contact is defined yet. Open an issue with
reproduction steps and affected version.
Проект находится на стадии MVP; контакт для безопасности пока не назначен.
Создайте issue с шагами воспроизведения и затронутой версией.
