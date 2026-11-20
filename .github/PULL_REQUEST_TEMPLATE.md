## What / Что сделано

<!-- Short summary of the change. / Краткое описание изменения. -->

## Why / Зачем

<!-- Motivation and related issue: Closes #123 / Обоснование и связанный issue. -->

## Verification / Проверка

- [ ] `npm run typecheck`
- [ ] `npm run build`
- [ ] `cargo check`
- [ ] `cargo test --lib`
- [ ] Loopback protocol tests passed (when core/transports changed)
- [ ] Manual test with a real/simulated Modbus device (if applicable)
- [ ] Cross-platform CI and Governance checks passed on the final PR commit
- [ ] Windows QA artifact verified for release-impacting changes (if applicable)

## Checklist / Чек-лист

- [ ] Docs updated (EN + RU) when behaviour or IPC changed
- [ ] `CHANGELOG.md` updated
- [ ] Protocol test vector added for codec changes
- [ ] Review was performed by someone other than the author/last pusher
- [ ] No merge before explicit reviewer approval
- [ ] Release approval / physical bench evidence is never inferred from CI
