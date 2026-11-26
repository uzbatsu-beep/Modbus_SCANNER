# Polling engine / Архитектура опроса

Target R20/R09/R19/R26. Preserve current transport behavior while extracting shared scheduling and ownership.

## Sessions and ownership
One connection manager owns physical resources. One master transaction stream per RS-485 segment/COM port (including discovery, monitoring and manual operations); queue or reject incompatible operations, never open competing masters. Independent serial ports and TCP endpoints may run concurrently with bounded workers. TCP gateway Unit IDs may share one physical downstream bus: configurable serialization/concurrency, conservative default. A failed endpoint must not stall unrelated sessions. Explicit start/stop, bounded cancellation, close/release on stop; retain existing authorization and write confirmations.

## User controls
- Period choices proposed: 50/100/250/500/1000 ms plus custom within validated limits. These are requested periods, not guaranteed throughput.
- Single: one logical tag/span per request (multiword tag remains atomic within one read).
- Group: compatible contiguous spans only, same connection/device/table, legal protocol/device quantity. User sets maximum block size; never bridge unsupported gaps by default.
- Adaptive: begin from declared ranges, split failed groups when evidence permits (e.g. illegal address), bound retries and remember learned constraints; never reinterpret timeout as absence. User can switch mode; no writes in monitoring.
- Separate timeout, inter-request gap, connection reuse/reconnect and retry policy. For serial respect wire time, turnaround and frame gaps; for TCP distinguish MBAP and RTU-over-TCP.

## Observability
Record transaction RTT with monotonic clock, connection establishment separately, queue delay, end-to-end cycle, actual sample interval, bytes, errors, missed deadlines. No fabricated zero RTT. Scheduling should avoid catch-up bursts when overloaded; report missed deadlines. Bounded queues and histories, incremental UI events.

## Acceptance
Fake clock and scripted transport: grouping preserves values and word order; errors remain attributable; single/group/adaptive selectable; stop interrupts waits and prevents new requests; independent endpoints progress while slow device times out; serial conflicts are blocked. Benchmark serial and TCP separately using [BENCHMARKS](BENCHMARKS.md).

PR #17 contract: [CONNECTION_MANAGER](CONNECTION_MANAGER.md). Phase A is isolated core; phase B integrates callers, phase C integrates Workbench. Existing scheduling changes remain deferred to PR #23.
