# Руководство по разработке

[English version](DEVELOPMENT.md)

## Требования

- Node.js 18+ и npm
- Rust 1.77+ (`rustup`), системный C-тулчейн (MSVC в Windows, `build-essential` в Linux,
  Xcode CLT в macOS)
- Системные библиотеки Tauri 2 — [официальный список](https://v2.tauri.app/start/prerequisites/)
- Windows: WebView2 Runtime (есть в Windows 11, на старых сборках ставится отдельно)

## Установка

```bash
npm install                 # зависимости фронтенда (Vue, Pinia, vue-router, Vite, Tauri CLI)
cd src-tauri && cargo check # загрузка и проверка Rust-зависимостей; создаёт Cargo.lock + gen/schemas
```

## Запуск

```bash
npm run dev            # только Vite (http://localhost:1420) — IPC бэкенда недоступен
npm run tauri dev      # Vite + окно Tauri с полным IPC (рекомендуется)
```

Первая сборка cargo скачивает ~300 крейтов (стек Tauri/wry/tokio) и занимает
несколько минут; последующие — инкрементальные.

## Сборка релиза

```bash
npm run tauri build
```

Артефакты — в `src-tauri/target/release/bundle/` (`.msi`/`.exe` в Windows,
`.deb`/`.AppImage` в Linux, `.app`/`.dmg` в macOS). После изменения иконок в
`tauri.conf.json` перегенерируйте их:

```bash
npm run tauri icon path/to/source.png
```

## Проверки

```bash
npm run typecheck        # vue-tsc: ошибки типов фронтенда
npm run build            # vue-tsc + production-бандл vite
cargo check              # в src-tauri/: проверка компиляции бэкенда
cargo test --lib         # юнит-тесты протокола (CRC, LRC, парсинг PDU)
```

**Известная проблема тулчейна (локальное окружение):** на Windows с `gnu`-тулчейном
Rust собранный тестовый бинарник завершается с `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`
(взаимодействие Tauri/WebView2 dynamic loader). Минимальный тестовый крейт с Tauri
работает, значит проблема возникает из-за полного набора зависимостей; `cargo check`
и `cargo build` работают нормально. Для запуска юнит-тестов в Windows используйте
`msvc`-тулчейн или запускайте тесты в CI/Linux.

## Соглашения проекта

### Rust

- Один модуль — одна ответственность; функциональные модули в
  `src-tauri/src/{modbus,transport,scanner,commands}`.
- Имена: `snake_case` для функций/типов/модулей, `SCREAMING_SNAKE_CASE` для констант.
- Всё, что пересекает IPC, derivatives `serde::Serialize`/`Deserialize` и лежит в
  `scanner/types.rs` (или в `commands/*` для локальных DTO команд).
- Ошибки — через `thiserror`; на границе команд они превращаются в `String`.
- Future движка должна оставаться `Send`: нельзя держать guard `parking_lot`
  через `.await`. Скопируйте данные в скоупе, затем ждите вне его.
- Долгие операции — в задаче tokio, а не в `#[tauri::command]`.
- Именуйте все функции/переходы, логируйте через `tracing`.

### TypeScript / Vue

- `<script setup lang="ts">` + Composition API; строгий TS включён.
- Pinia для общего состояния; не дублируйте состояние бэкенда вне черновиков форм.
- Весь IPC — только через `src/api/mod.ts`; компоненты не вызывают `invoke` напрямую.
- Любое изменение Rust IPC-типа требует ручного обновления `src/types/ipc.ts`
  (генератор запланирован).
- Сейчас пользовательские строки смешаны RU/EN; локализация — в ROADMAP.

## Частые задачи

### Добавить команду Tauri

1. Напишите `#[tauri::command] pub async fn ...` в `src-tauri/src/commands/*.rs`.
2. Зарегистрируйте в `generate_handler!` в `lib.rs`.
3. Добавьте новые типы аргументов/результата в `src/types/ipc.ts`.
4. Добавьте типизированную обёртку в `src/api/mod.ts`.

### Добавить код функции

1. Добавьте его в `FunctionCode` в `modbus/frame.rs`, сборщики и парсеры.
2. Если это зонд — расширьте `probe_pdu()` в `scanner/strategies.rs`.
3. Обновите `FC`/`FC_NAMES` в `src/types/ipc.ts` и опции Manual-представления.
4. Добавьте тест-вектор протокола.

### Изменить поведение сканирования

См. `src-tauri/src/scanner/strategies.rs` (перебор целей),
`scanner/engine.rs` (исполнение) и `scanner/types.rs` (схема параметров).
При изменении контракта обновите docs/API и ROADMAP.

## Карта файлов

| Путь | Назначение |
|------|------------|
| `src-tauri/src/modbus/` | CRC/LRC, PDU/ADU, сборщики и парсеры FC |
| `src-tauri/src/transport/` | Async I/O: serial / TCP / UDP |
| `src-tauri/src/scanner/` | Стратегии, движок, IPC-типы |
| `src-tauri/src/commands/` | Обработчики команд Tauri |
| `src-tauri/src/state.rs` | `AppState`, `Config` |
| `src/types/ipc.ts` | TS-зеркало IPC-контракта |
| `src/api/mod.ts` | Обёртки invoke/listen |
| `src/stores/` | Сторы Pinia |
| `src/views/`, `src/components/` | UI |
