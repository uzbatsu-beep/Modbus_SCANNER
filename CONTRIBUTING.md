# Contributing / Участие в разработке

Thanks for your interest in Modbus Scanner! / Спасибо за интерес к проекту!

## Getting started / Начало работы

1. Read [README.md](README.md) and [DEVELOPMENT.md](docs/DEVELOPMENT.md).
2. Fork the repository and create a branch from `main`.
3. Set up the toolchain: Node.js 18+, Rust 1.77+, Tauri 2 prerequisites.
4. Make changes, then run the checks:

```bash
npm run typecheck
npm run build
cd src-tauri && cargo check && cargo test --lib
```

## Conventions / Соглашения

- Keep Rust IPC types and `src/types/ipc.ts` in sync.
- All IPC calls go through `src/api/mod.ts`.
- Add a protocol test vector when changing framing/parsing.
- Update the relevant files in `docs/` (EN and RU versions) and `CHANGELOG.md`.
- Write commit messages in English, imperative mood (`Add baud sweep strategy`).

## Pull requests / Пулл-реквесты

- Describe what changed and how you verified it.
- Reference related issues (`Closes #123`).
- CI must be green before merge.
- Keep PRs focused; split unrelated changes into separate PRs.

## Reporting bugs / Сообщения об ошибках

Use the [bug report template](https://github.com/uzbatsu-beep/Modbus_SCANNER/issues/new?template=bug_report.yml).
Include OS, version, transport (RTU/ASCII/TCP/UDP), device model, exact steps
and logs (`RUST_LOG=debug`).

Используйте [шаблон bug report](https://github.com/uzbatsu-beep/Modbus_SCANNER/issues/new?template=bug_report.yml):
укажите ОС, версию, транспорт, модель устройства, шаги воспроизведения и логи.

## License / Лицензия

By contributing you agree that your contributions are licensed under
[GNU GPL v3.0](LICENSE).

Отправляя изменения, вы соглашаетесь с лицензией [GNU GPL v3.0](LICENSE).

## Documentation and OT acceptance / Документация и приёмка

- Update paired EN/RU reference pages when changing behavior, API, UI, transport or safety; navigate via [INDEX](docs/INDEX.md).
- Run `npm run docs:check` to test the Markdown checker and verify **local** paths. This does not verify anchors, external links or technical accuracy.
- Every PR must pass existing CI checks. Windows QA artifacts are **not releases** and do not replace real-device field validation.
- Never infer current Ruleset requirements from JSON in Git alone; check the active GitHub settings and [dated audit](docs/AUDIT_2026-11-21.md).

- Обновляйте русскую и английскую документацию вместе с изменениями кода.
- Перед PR выполняйте `npm run docs:check`; проверка не заменяет техническую экспертизу.
- Не используйте тестовые установщики Windows как доказательство успешных стендовых испытаний.
