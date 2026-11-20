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

## Tests (E-08)

Unit tests cover the protocol layer (CRC/LRC, PDU/ADU, decoding), scan
strategies (baud sweep, register blocks, diff) and the engine pipeline with an
in-memory `MockTransport` (`src-tauri/src/transport/mock.rs`) — no hardware or
network required. CI runs `cargo test --lib` on **Windows, Linux and macOS**;
frontend typecheck/build and generated-binding freshness are verified on every
push as well.

On a local Windows-gnu toolchain the test executable may fail to start with
`0xc0000139` (WebView2 loader). Use `cargo test --lib` on the MSVC toolchain
or rely on CI.

## Generated IPC types (E-07)

`src/types/ipc.ts` is a facade over `src/types/generated/`, which is generated
from Rust with `ts-rs`. After changing any IPC type run:

```bash
npm run gen:types     # cd src-tauri && cargo run --bin gen_bindings
npm run typecheck
```

CI regenerates the bindings and fails if `src/types/generated/` is out of date.
Only UI constants (`FC`, `FC_NAMES`, `BAUD_RATES`) live in `ipc.ts` by hand.
`u64`/`u128` fields are annotated with `#[ts(type = "number")]` because JSON
serializes them as JavaScript numbers.

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

## Windows QA installers (PR #3)

Before field acceptance, use the dedicated **Windows QA installers (no release)**
GitHub Actions workflow. It produces NSIS EXE + MSI for Windows, captures
SHA256 checksums and the source commit, and uploads a time-limited test artifact
**without** creating a GitHub Release or a tag. See
[WINDOWS_QA.md](WINDOWS_QA.md) and the
[hardware acceptance checklist](FIELD_VALIDATION_v1.0.1.md).
Both QA and official release builds verify version manifest consistency.

## Release process (E-06)

The official release is **manual, gated, and fails closed**. Do **not**
publish by pushing a `v*` tag: this event no longer triggers a Release.

1. Prepare all matching version manifests and `CHANGELOG.md` in an approved
   PR, merge it to `main`, and wait for successful CI + Windows QA on the
   **exact merged main SHA**.
2. Complete isolated physical hardware acceptance and attach the signed-off
   evidence to a repository GitHub Issue.
3. Confirm active branch and tag Rulesets, and a `production-release`
   environment with independent required reviewers, prevent self-review,
   main-only deployment and `OT_RELEASE_APPROVAL_GATE` environment secret.
4. Manually run **Release (manual, protected)** on `main` with tag, QA run ID,
   bench PASS flag and GitHub issue URL. The preflight verifies version,
   main SHA, 5 CI checks, QA artifact, rulesets and evidence; the Windows
   builder packages EXE/MSI with SHA256 and build metadata.
5. An independent GitHub reviewer approves the `production-release` job.
   Only then can GitHub Release and its new tag be created.

See [release governance (EN)](RELEASE_GATES.md) and
[release governance (RU)](RELEASE_GATES.ru.md) for detailed activation
instructions. A committed ruleset JSON is not an active GitHub setting.

Local equivalent: `npm run tauri build`; artifacts are in
`src-tauri/target/release/bundle/{msi,nsis}/`. The Windows target requires
WebView2 Runtime on the end user's machine.
