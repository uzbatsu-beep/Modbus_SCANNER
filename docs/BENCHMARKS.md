# Performance measurement / Производительность

Targets are hypotheses, not measured results. Compare old/new implementations on identical hardware, workload and settings. Separate serial, MBAP TCP and RTU-over-TCP.

## Matrix
- 1/7/32/125 contiguous holding registers, plus sparse ranges and mixed types/tables.
- One device; several units on one shared serial bus; independent TCP endpoints and independent serial ports.
- Requested periods 50/100/250/500/1000 ms where permitted; single/group/adaptive modes; healthy, slow and timeout endpoint.
- Record device model, firmware, baud/parity/stop, frame gaps, timeout, retries, OS, app SHA and simulator vs physical bench.

## Metrics
Actual sample interval p50/p95/max, transaction RTT p50/p95/p99, connect/queue time, cycle duration, successful/error counts, bytes, missed deadlines, throughput, memory and UI responsiveness. Report elapsed time using monotonic clock; distinguish estimated wire utilization from measured utilization. Include warm-up, sample count and raw results. No extrapolation from TCP to RS-485, no guaranteed 50 ms under overload.

Acceptance: no semantic value changes across poll modes; no dropped quality/error records without explicit indication; bounded buffers; no new TX after completed Stop; no competing master on shared bus. Establish baseline before setting numerical speedup targets.
