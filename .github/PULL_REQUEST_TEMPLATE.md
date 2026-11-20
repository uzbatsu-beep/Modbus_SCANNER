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
- [ ] Repository Ruleset still requires a PR, all 5 CI checks and clean review threads
- [ ] Maintainer has explicitly approved merging this PR (no mandatory external reviewer)
- [ ] Release remains manual; independent hardware bench PASS evidence is not inferred from CI
