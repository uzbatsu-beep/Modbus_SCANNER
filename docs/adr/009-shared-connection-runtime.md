# ADR-009 — Shared Connection Runtime

Status: maintainer approved, PR #21. Variant B; TCP idle 60 s, serial idle 30 s; 32 unused cached connections; resource-level recovery; migrate Monitor transport without scheduler rewrite; serial discovery rejects busy port; C.1–C.4 and regression gates; no merge before verification.

Decision: distinguish resource, transport configuration, worker generation, client lease and operation scope. Compatible acquisitions atomically reuse one worker; incompatible transport/policy rejected. Explicitly registered workers are pinned; automatic workers are eligible for idle expiry only without leases and queued/active work. Retire by marking stop under registry lock, then join before releasing ownership. Leases refer to a specific entry generation, not a reusable string ID. TCP opening timeout is still a connection setting; response timeout is per transaction. UDP uses network idle 60 s. No silent TLS/framing downgrades.

Resource recovery deadlines use monotonic time and survive worker removal; a new worker waits even if framing changed. Expired deadlines pruned. No automatic retransmission. Direct legacy serial I/O must consume the resource recovery state in C.2; no claim of arbitrary late-response exclusion.

C.1 provides isolated lifecycle API and tests. C.2 migrates serial/sweep/COM test, C.3 migrates known-device Scanner/Manual/Monitor and protocol configuration, C.4 performs end-to-end acceptance. Independent scheduling is B.3, UI/persistence 17.C. See [contract](../SHARED_CONNECTION_RUNTIME.md).
