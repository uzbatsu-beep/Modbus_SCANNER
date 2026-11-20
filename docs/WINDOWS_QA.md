# Windows QA builds (PR #3)

[Русская версия](WINDOWS_QA.ru.md)

## Purpose

The `Windows QA installers (no release)` workflow builds **both** Windows
installers from the exact reviewed source commit:

- NSIS `.exe`
- Windows Installer `.msi`

It runs the protocol regression tests, checks version consistency across
`package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock` and
`tauri.conf.json`, and builds with the same `tauri build` toolchain as the
published release workflow. This process **does not** create a Git tag,
publish a GitHub Release, or claim hardware/field validation.

## Obtain the QA installers

1. Open [GitHub Actions → Windows QA installers](../.github/workflows/windows-qa.yml)
   and select the successful PR build; after merge, you can also use **Run workflow**
   on the default branch.
2. In the run's **Artifacts** section, download the
   `modbus-scanner-v1.0.1-windows-qa-<run>` artifact. GitHub may require sign-in.
3. Extract the archive to inspect the NSIS EXE, MSI,
   `SHA256SUMS.txt`, and `BUILD-INFO.txt`. Match the source SHA with the
   approved commit. The artifact is retained for **14 days**.
4. On Windows 10/11, verify SHA-256:
   `Get-FileHash .\Modbus.Scanner_1.0.1_x64-setup.exe -Algorithm SHA256`
   and compare its hex digest with the accompanying checksum file.
5. Test on an **isolated** Modbus bench. Follow
   [FIELD_VALIDATION_v1.0.1.md](FIELD_VALIDATION_v1.0.1.md) and record actual
   FC03/FC04 responses, errors and RS-485 timing before considering a release.

**Important:** This is an unsigned QA installer. Windows SmartScreen may display
a warning; verify GitHub source and SHA256 before executing it. Do not lower
system security settings or use this build in an uncontrolled OT network.

## Release gate

A green GitHub Actions run verifies that packaging and automated tests work.
It does **not** verify electrical wiring, serial adapters, actual PLC/VFD
responses, device-specific register maps, or process safety.

Only after field acceptance and explicit approval should the team create the
`v1.0.1` tag. The tag-triggered `release.yml` is separate and publishes
the official GitHub Release. Never use the QA artifact as an official release.
