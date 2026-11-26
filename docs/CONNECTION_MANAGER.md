# Connection Manager — PR #17

Approved architecture and policy: [ADR-008](adr/008-connection-manager.md).

## Phase 17.A — core
New module transport/connection.rs: serial/TCP/UDP definitions, lazy registry, native transport factory, one Tokio worker per resource, 128 waiting jobs plus one active, fair weighted queue, bounded backpressure, completed-transaction timing and queue wait. Disconnect is an out-of-band stop; waiting jobs are cancelled and resource lease persists until worker join. Active transaction may finish before Stop returns; open bound 20 s, transaction bound response timeout + 2 s (maximum 62 s), close bound 1 s; total depends on current stage. No automatic replay after error. Error closes transport and enables demand-driven reconnect with backoff. Connection status and counters are runtime-only in phase A. Caller is responsible for validated ADU, authorization and protocol response checks.

Tests: duplicate resource rejection, serial framing independence, priority fairness/FIFO, Persistent/OnDemand, reconnect without replay, queue capacity, disconnect, abandoned queued caller, disabled reconnect, independent endpoint, and native TCP loopback socket reuse across two Unit IDs. CI must compile and execute tests on Windows, Linux and macOS.

## Phase 17.B — pending
Route existing operations through common ownership; exclusive serial autodiscovery; explicit cancellation and late frame handling; unify framing and TLS configuration; retain per-operation validation and existing safety policy; independent per-resource polling; queue metrics, logs and offline loopback integration. Real physical topology remains explicit, not guessed.

## Phase 17.C — pending
Persist connection definitions, generated IPC, editor and lifecycle controls in Workbench, tool selectors, current-state diagnostics, short localized menus. Full device ID/project workspace transition remains PR #18.

## RU
Утверждены вариант B, Persistent TCP, автоматическое переподключение, очередь 128, приоритеты без голодания, три этапа и исправления UI. На 17.A менеджер изолирован от существующих вызовов; подключение сканера, мониторинга, ручного режима и UI выполняется на 17.B/17.C. Автоматические тесты не заменяют стендовые. Не сливать незавершённый PR.
