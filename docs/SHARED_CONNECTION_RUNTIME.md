# PR #21 — Shared Connection Runtime

Approved B.2.3 variant B: TCP idle 60 s, Serial idle 30 s, 32 unused cached connections, resource recovery enabled, Monitor transport integration without new scheduler, busy serial auto discovery refused, C.1–C.4, no merge until review. PR #20 is merged.

## C.1 — lifecycle foundation
- ConnectionManager.acquire returns a generation-bound ConnectionLease. Compatible physical resource and serialized settings reuse the same worker; mismatched framing, TLS, opening timeout or connection policy rejected. Unit ID/response timeout are request parameters. Explicit entries remain pinned; automatic entries receive internal IDs.
- Each lease has its own OperationScope. Stop/cancellation does not affect other leases. Drop cancels own pending requests; active work completes within existing bounds. Explicit Disconnect retires the entire worker. Old leases cannot submit to a replacement entry.
- Automatic idle TCP and UDP 60 s, serial 30 s, counted from last release; no eviction of leased or queued/active workers. Oldest quiescent idle workers retired above cap 32; closing workers may exist transiently until joined. One weak-reference reaper per manager, 250 ms tick; expiry is not a hard real-time deadline. Drop also checks capacity. Shutdown joins registered workers; production shutdown wiring remains C.3.
- RecoveryBook stores non-MBAP failure deadlines by physical resource even after disconnect. New workers wait before open/TX, including different framing. Existing retry backoff remains. Expired deadlines removed; direct serial leases do not yet enforce this state. No replay.
- Regression scenarios: compatible reuse, scoped stop, incompatible framing/policy, idle eviction vs pinned/leased, cache cap, stale lease isolation, serial exclusion, cross-generation recovery.

## C.2 — PR #22 in progress
Managed serial scanner/FC43, COM diagnostics, exclusive sweep reconfiguration, safe scoped Stop and recovery enforcement for all serial paths. Idle cached serial ports currently remain owned until eviction; define atomic idle retirement before exclusive discovery while refusing actively leased ports.

## C.3 — pending
Known-device persistent acquisition in Scanner/Manual/Monitor, common target framing/TLS, transaction ID generation, raw single-shot semantics, transport-only Monitor migration, shutdown integration; transient bounded IP discovery retained. Resolve connection-open timeout configuration consistently; do not alter read/write authorization or Quiet Mode.

## C.4 — pending
Full cross-tool regression, real loopback and controlled serial tests, no direct tool opens except documented exclusive manager-owned discovery implementation, all CI and Windows QA, documentation and explicit review. Physical bench acceptance separate.

## RU
C.1 — фундамент аренды соединений и сохранения Recovery между worker. Реальный перевод Scanner, Manual, Monitor и последовательной диагностики выполняется на C.2/C.3. Независимые циклы Monitor — B.3. Кэш не удерживает более 32 готовых к удалению неиспользуемых подключений; активные и явно зарегистрированные не вытесняются. Пауза снижает риск запоздалого ответа, но не исключает произвольные задержки. Не считать PR готовым по одному C.1.

Additional native TCP regression: three Unit IDs from separate leases use one socket, including after all leases are released and a new lease is acquired; shutdown produces EOF. A periodic-reaper test verifies expiry without a subsequent acquisition. The labels Scanner/Manual/Monitor in this fixture simulate clients of the lifecycle API, not the existing production tool entry points.

C.1 merged in PR #21 with 151 Rust tests per platform and Windows QA. PR #22 C.2 approved; see [SERIAL_MIGRATION](SERIAL_MIGRATION.md). C.2.1 introduces exclusive session and atomic cache handoff; legacy entry points remain pending.
