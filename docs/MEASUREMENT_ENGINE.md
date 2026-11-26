# Measurement Engine / PR #14

Approved variant B: 10,000 observations/channel by default, 200 ms UI batching, last good with explicit quality on errors, Logger v2. No new dependencies or backward compatibility requirement.

- One canonical Rust decoder; 16-bit BADC/DCBA swap bytes, ABCD/CDAB leave word intact; 32-bit ABCD/BADC/CDAB/DCBA. Numeric scaling rejects nonfinite and zero divisor. ASCII retains existing printable text semantics.
- Frozen MonitorWatch (tag ID, source, spec); session identifier and per-watch sequence; monotonic elapsed microseconds and UTC milliseconds. Separate raw, decoded value, quality, exception code and detail. Failed sample is a gap; last_good never logged as a new success.
- RAM VecDeque; adjustable 100–50,000/channel, total <=1,000,000 observations. Limit is sample count, not a measured RAM bound. All-session good numeric MIN/MAX/AVG/delta; count valid text but no numeric stats. No persistence of RAM history. RAW watch one word; multiword typed span one request. No polling optimization in this PR.
- Independent periodic 200 ms delta emitter; full snapshot for initial state, gap recovery and Stop; final event flush. UI merge by sequence and session, history bounded. Trend uses monotonic time, signed range, gaps for failure/missing sequence; single displayed series. Last-good age visible; stopped values stale. Detailed live staleness thresholds and communication metrics remain later work.
- Logger worker bounded queue 4096 observations, nonblocking enqueue. One row per observation with raw + typed + quality (failed values empty); session/watch/sequence identifies record. Pending/dropped counts are shared queue counters, failed/written per logger. Config replacement discards queued older generation and counts drops. File IO errors counted, not retried. Wait for pending=0 before closing app for complete logs; no crash durability promise. Separate .v2 paths and actual session-based rotation. CSV/TSV quote fields; JSONL includes watch metadata. No RTT placeholder.
- Existing poll delay remains 250–10,000 ms (not measured cycle). Transport ownership and higher rates are PR #16/#21; RTT/period diagnostics PR #15. Expanded types PR #17; multi-series PR #23.

Tests: Rust signed/order/nonfinite, ring eviction, timeout/recovery, text stats, Logger header and records; frontend incremental merge and negative time-axis gaps. CI and physical verification are separate.

## RU
Новая модель измерений не подменяет отсутствие данных нулём. При ошибке отображается последнее достоверное значение с качеством текущего наблюдения; в журнале и тренде фиксируется именно ошибка. История ограничена числом измерений, UI получает только новые пакеты, полное состояние запрашивается при восстановлении. Запись на диск вынесена из цикла опроса; переполнение очереди отображается в диагностике. Старый формат логов не поддерживается. Физические показатели производительности ещё не установлены.
