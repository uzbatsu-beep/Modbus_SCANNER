# PR #22 — C.2 Serial Migration

Approved variant B Managed Serial Runtime; atomic Idle Cache release before discovery; Port Test separate from Modbus Communication Test; configurable unit ID, FC01–FC04, address, quantity and timeout (defaults 1, FC03, 0, 1, 800 ms); valid exception confirms communication but not read success; virtual serial test when available; internal serial diagnostics; C.2.1–C.2.5, SR-01–SR-16; no merge before review.

## C.2.1 — exclusive foundation
ExclusiveSerialSession reserves physical serial key under registry lock before retiring idle automatic entry, holds guard across reconfiguration, cancellation and cleanup. Reject active/pinned resources, other reservations, queued/active transactions. Matching configuration reuses worker; incompatible target rejected; reconfiguration joins old worker, retains recovery deadline, then creates new lazy worker. Internal snapshot reports owner, resource key, worker ID, actual selected config/mode, count of configurations applied (including initial selection), remaining recovery. No physical open until first transaction. Scoped Stop blocks subsequent configuration and requests. Drop performs cancellation-safe worker cleanup. Existing production serial tools still use previous paths; this is the foundation for subsequent commits.

## C.2.2 — pending
Fixed-configuration Serial Scanner and FC43 use normal shared lease, common validator, timing/traffic, safe scoped Stop. Legacy monitor remains pending C.3.

## C.2.3 — pending
Wire serial sweep into ExclusiveSerialSession, preserve existing scan strategies, stop-on-first and rate limits. Recovery remains physical-resource scoped; no automatic write replay.

## C.2.4 — pending
Managed no-TX Port Test and configurable read-only Communication Test, exception classification, UI and generated IPC bindings. Preserve current defaults; validate range and function before I/O.

## C.2.5 — pending
Integrated tests, diagnostics audit, virtual serial where available, cross-platform CI, Windows QA; explicit maintainer review. Physical bench separate.

## Acceptance matrix
| ID | Criterion |
|---|---|
| SR-01 | compatible COM worker reuse |
| SR-02 | incompatible settings rejected |
| SR-03 | Unit ID change does not reopen |
| SR-04 | discovery refuses active lease |
| SR-05 | atomic idle cache handoff |
| SR-06 | reservation survives framing change |
| SR-07 | recovery survives baud change |
| SR-08 | Stop prevents next transaction |
| SR-09 | safe active completion |
| SR-10 | RTU CRC |
| SR-11 | ASCII LRC |
| SR-12 | FC43 |
| SR-13 | valid exception confirms communication |
| SR-14 | no TX in Port Test |
| SR-15 | no leaked resource |
| SR-16 | independent COM resources |

C.2.1 adds MockTransport and registry tests for SR-04/05/06/07/08/15/16 at API level. Full production acceptance and virtual/physical serial tests not yet established.

## RU
Утверждён вариант B. C.2.1 реализует исключительную сессию и передачу порта из кэша без окна конкурентного доступа. Обычное сканирование и диагностика будут переведены на управляемый транспорт на C.2.2–C.2.4. Проверки C.2.5 обязательны перед слиянием. Прямые legacy-вызовы пока сохранены.

Two additional gated-transport regressions cover cancellation during idle-worker close (reservation remains until join) and Stop during active RX with a queued request (only active TX finishes). Snapshot adds lease count, queue depth, active requests and reconnect count; queue_wait_us remains per TransactionOutcome. Configuration fields represent selected settings, not proof of successful physical opening. Seven new tests; virtual serial and physical RS-485 are not exercised by these fixtures.

C.2.1 merged via PR #22. C.2.2–C.2.5 remain pending. Windows-first CI policy approved after merge; see [release gates](RELEASE_GATES.md). Final three-platform testing remains mandatory for release; virtual serial/physical bench separate.
