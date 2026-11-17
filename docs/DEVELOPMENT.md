# Development guide

[Русская версия](DEVELOPMENT.ru.md)

## Prerequisites

- Node.js 18+ and npm
- Rust 1.77+ (`rustup`), a C toolchain (MSVC on Windows, `build-essential` on Linux,
  Xcode CLT on macOS)
- Tauri 2 system libraries — see the [official prerequisites](https://v2.tauri.app/start/prerequisites/)
- Windows: WebView2 Runtime (present on Windows 11; install separately on older builds)

## Setup

```bash
npm install                 # frontend deps (Vue, Pinia, vue-router, Vite, Tauri CLI)
cd src-tauri && cargo check # fetch and check Rust deps; generates Cargo.lock + gen/schemas
```

## Running

```bash
npm run dev            # only Vite (http://localhost:1420) — backend IPC will be unavailable
npm run tauri dev      # Vite + Tauri window with full IPC (recommended)
```

The first `cargo` build downloads ~300 crates (Tauri/wry/tokio stack) and takes
several minutes. Subsequent builds are incremental.

## Building for release

```bash
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/` (`.msi`/`.exe` on Windows,
`.deb`/`.AppImage` on Linux, `.app`/`.dmg` on macOS). If you change
`tauri.conf.json` icons, regenerate them with:

```bash
npm run tauri icon path/to/source.png
```

## Verification

```bash
npm run typecheck        # vue-tsc: frontend type errors
npm run build            # vue-tsc + vite production bundle
cargo check              # in src-tauri/: compile check for backend
cargo test --lib         # protocol unit tests (CRC, LRC, PDU parsing)
```

**Known toolchain issue (local environment):** with the Windows `gnu` Rust
toolchain, the built test binary exits with `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`
(Tauri/WebView2 dynamic loader interaction). A minimal Tauri test crate runs
fine, so this is triggered by the full dependency set; `cargo check` and
`cargo build` work normally. Use the `msvc` toolchain for running unit tests on
Windows, or run tests in CI/Linux.

## Project conventions

### Rust

- One module per concern; feature modules live under `src-tauri/src/{modbus,transport,scanner,commands}`.
- Names: `snake_case` functions/types modules, `SCREAMING_SNAKE_CASE` constants.
- Everything crossing IPC derives `serde::Serialize`/`Deserialize` and lives in
  `scanner/types.rs` (or `commands/*` for command-local DTOs).
- Prefer `thiserror` error enums; commands convert to `String` at the boundary.
- The engine future must stay `Send`: never hold a `parking_lot` guard across
  `.await`. Copy/clone the data inside a scope, then await outside.
- Long operations belong in the tokio task, not in a `#[tauri::command]`.
- Name every feature/transition; log with `tracing`.

### TypeScript / Vue

- `<script setup lang="ts">` + Composition API; strict TS settings are on.
- Pinia stores for shared state; never duplicate backend state locally beyond a
  draft form.
- All IPC access goes through `src/api/mod.ts`; components never call `invoke`
  directly.
- Any change to a Rust IPC type requires updating `src/types/ipc.ts` manually
  (generator planned).
- User-facing strings are currently RU/EN mixed; localization is on the ROADMAP.

## Common tasks

### Add a Tauri command

1. Write `#[tauri::command] pub async fn ...` in `src-tauri/src/commands/*.rs`.
2. Register it in `lib.rs` `generate_handler!`.
3. Add its argument/result types to `src/types/ipc.ts` if new.
4. Add a typed wrapper in `src/api/mod.ts`.

### Add a function code

1. Add it to `FunctionCode` in `modbus/frame.rs` and the builders/parsers.
2. If it is a probe, extend `probe_pdu()` in `scanner/strategies.rs`.
3. Update `FC`/`FC_NAMES` in `src/types/ipc.ts` and the Manual view options.
4. Add a protocol test vector.

### Change scan behaviour

See `src-tauri/src/scanner/strategies.rs` for target enumeration,
`scanner/engine.rs` for execution, and `scanner/types.rs` for the parameter
schema. Update docs/API + ROADMAP when the contract changes.

## File map (quick)

| Path | Purpose |
|------|---------|
| `src-tauri/src/modbus/` | CRC/LRC, PDU/ADU, FC builders and parsers |
| `src-tauri/src/transport/` | serial / TCP / UDP async I/O |
| `src-tauri/src/scanner/` | strategies, engine, IPC types |
| `src-tauri/src/commands/` | Tauri command handlers |
| `src-tauri/src/state.rs` | `AppState`, `Config` |
| `src/types/ipc.ts` | TS mirror of IPC contract |
| `src/api/mod.ts` | invoke/listen wrappers |
| `src/stores/` | Pinia stores |
| `src/views/`, `src/components/` | UI |
