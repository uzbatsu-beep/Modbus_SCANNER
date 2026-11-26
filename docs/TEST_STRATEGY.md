# Test strategy / Проверки

Prototype focus: deterministic engineering correctness, quick feedback, physical acceptance distinct from CI. No automatic publishing or new approval bureaucracy.

| Layer | Required evidence |
|---|---|
| Protocol | Golden ADU/PDU, exact FC/unit/transaction checks, malformed/fragmented frames, CRC/LRC, exception codes |
| Model | FC03 vs FC04 same address; two devices; schema v2 round trip; v1/future rejection; valid zero vs missing word; profile round trip |
| Decoder | Signed boundaries, endian permutations, exact 64-bit values, scaling, enums, invalid input |
| Scheduler | Single/group/adaptive equality, serial ownership, independent endpoint progress, deadlines, cancellation and reconnect |
| UI/IPC | Generated bindings, consistent raw/typed/quality display, negative trend, stale/invalid not zero |
| Simulator | Scripted deterministic success, exception, timeout and corrupted response cases on isolated loopback |
| Physical | Approved isolated RTU/ASCII/TCP bench, real driver/turnaround, actual timing, recorded firmware and results |

Use mock transport and existing loopback simulator first. For physical tests follow [FIELD_VALIDATION_v1.0.1](FIELD_VALIDATION_v1.0.1.md) and [release gates](RELEASE_GATES.ru.md); no invented PASS. Keep five existing CI checks; Windows QA remains separate. New behavior requires positive and negative regression tests. Stop discovery/monitor before physical cable or settings changes. Record exact source SHA, toolchain, environment and failures.
