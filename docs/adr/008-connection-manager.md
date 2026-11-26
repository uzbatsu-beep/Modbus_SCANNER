# ADR-008 — Connection & Transport Manager

Status: approved by maintainer, 2026-11-26. Variant B; TCP Persistent by default; auto reconnect enabled; 128 waiting requests per connection; priority scheduling without starvation; stages 17.A → 17.B → 17.C; targeted UI improvements approved. No merge until complete review.

Context: independent direct transport opens in scanner, monitor, manual and serial test conflict on physical resources and duplicate configuration. Keep existing protocol codecs, measurement/quality, timing and authorization.

Decision: one worker per owned resource; independent workers; bounded queue, FIFO per class with weighted round robin (3 high, 2 normal, 1 low); control outside queue. Serial key excludes framing and baud. TCP/UDP key excludes Unit ID. Windows COM aliases and existing Unix canonical paths normalized; DNS aliases, alternate network paths and physical gateway topology cannot be inferred automatically. One process scope, not protection from other applications.

Persistent vs OnDemand are lifetime modes, reconnect is separate. Reconnect on subsequent request after failure/backoff (500 ms doubling to 5 s); never replay a submitted transaction automatically. Lazy registration in 17.A; actual transport state is not proof of a responding slave. Disconnect stops queued work, lets the active bounded operation finish, then closes and joins before releasing ownership. Abandoned queued callers are skipped. A completed write with missing acknowledgement remains uncertain.

17.A: transport-independent worker, native transport factory, definitions/runtime state, tests. 17.B: integrate all callers including scanner/monitor/manual/serial test, CLI/API, exclusive serial discovery lease, framing/validation, per-connection metrics and Logger, independent polling, shared authorization. 17.C: IPC, saved definitions and Workbench UI, localization cleanup, complete end-to-end regression tests. Do not claim integration until 17.B and 17.C are verified.

Tradeoff: no request pipelining, adaptive polling, full workspace migration, background probing or automatic retransmission. Explicit topology for gateways and electrical timing remain separate. Existing direct transport paths are unchanged in 17.A, so ownership is not yet application-wide. See [CONNECTION_MANAGER](../CONNECTION_MANAGER.md).
