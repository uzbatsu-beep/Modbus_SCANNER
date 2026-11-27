# PR #25 — C.3 Protocol & Cross-Tool Integration

Maintainer approved variant B, TCP connect timeout 2000 ms, exclusive Raw Frame, seven protocol modes, separate RTT/queue wait, centralized shutdown, reset-to-TX cancellation gate, Monitor unchanged scheduler, WRR 3:2:1, queue 128, reserve 8, C.3.1–C.3.7, CT-01–CT-16, outstanding C.2.5 in C.3/C.4, Windows-first and Windows QA. No additions. No merge until review.

## C.3.1 — foundation checkpoint
Common configuration constructors reuse TransportConfig, with five typed NetworkProtocol variants and SerialMode. Default TCP opening timeout 2000 ms, default TLS port 802, explicit ports preserved. Reject RTU+TLS; same resource with incompatible configuration is rejected. Existing serial scanner uses common serial definition. IPC target extension, propagation of framing/TLS through discovered devices, project/watch/UI state, migration of other callers pending. Existing discovery behavior unchanged.

## C.3.2 — foundation checkpoint
Shared PreparedRequest path for explicit and leased CommunicationService execution: validates structured request, constructs one ADU and fresh transaction ID per attempt, validates response inside worker, preserves raw rejected ADU and measured timing/queue wait. No automatic retries; caller must apply authorization. Tests: real TCP loopback sharing socket and different Unit IDs/timeouts with independent lease Stop, malformed request before open, seven-mode codec/validator fixture (not seven physical transports or TLS handshake). Serial/UDP/RTU/TLS native end-to-end tests still required.

## C.3.3–C.3.5 — pending
Manual shared structured reads/writes with fresh IDs and no write replay; exclusive single-shot raw for all resources. Persistent fixed Scanner TCP/UDP; transient IP range discovery and resource cap. Monitor shared leases, preflight all resource settings, no scheduler change; preserve quality/history/metrics. Propagate connection settings end to end; reject incompatibility, no silent TLS downgrade.

## C.3.6 — partial
Worker now uses logical TX admission gate after reset and before send_raw, synchronized with OperationScope.cancel. Caller drop/worker Stop checked at boundary. Once admitted, bounded I/O completes. OnDemand pre-TX cancellation closes opened transport. Regression gates Reset, cancels lease before releasing Reset, checks no TX and other lease works. This is a software commitment point, not a hardware guarantee that the first bit is already transmitted; serial driver gap/partial TX limits remain. Scanner RTT migration, queue wait diagnostics, worker-level quiet semantics, centralized shutdown and further SR tests remain pending.

## C.3.7 — pending
Windows CI/QA, exact SHA, integrated production tool tests; C.4 full B.2 acceptance, virtual COM where available, physical bench separately. No new platform build claims.

## Acceptance matrix
| ID | Criterion |
|---|---|
| CT-01 | one COM worker for Scanner/Manual/Monitor |
| CT-02 | one TCP socket across tools |
| CT-03 | Unit ID independent of connection |
| CT-04 | response timeout independent of connection |
| CT-05 | incompatible framing/TLS rejected |
| CT-06 | seven protocol modes |
| CT-07 | independent Stop |
| CT-08 | fresh MBAP ID per attempt |
| CT-09 | no write/raw replay |
| CT-10 | raw isolation |
| CT-11 | measured RTT and queue wait |
| CT-12 | no resource leaks |
| CT-13 | bounded IP discovery |
| CT-14 | shutdown |
| CT-15 | preserve monitor quality/history |
| CT-16 | Windows CI and QA |

## RU
Утверждён вариант B. Первый коммит — общий конфигуратор транспорта, подготовка PDU/ADU для арендованного worker и граница отмены после Reset. Это не завершённая интеграция инструментов; C.3.3–C.3.7 и IPC остаются открыты. Физические и виртуальные испытания не выполнены.

Serial Diagnostics structured request now calls execute_leased, and both Serial Scanner and Serial Diagnostics construct identical serial settings through the common constructor. Existing four diagnostic regression functions exercise this production call; IPC shape unchanged. Manual, network Scanner and Monitor still pending.

## C.3.1 — caller configuration follow-up (2026-11-27)
Manual now constructs its legacy RTU/ASCII, TCP MBAP and UDP RTU definitions with the shared configuration constructors; all three production callers (Manual, Scanner, Monitor) use the 2000 ms default TCP connect timeout independently of the response deadline. Manual remains an OnDemand temporary worker; this change does not claim shared Manual, exclusive Raw migration, or full seven-mode IPC propagation. Added a regression for Manual TCP connect timeout and explicit port. Existing scheduler, retry policy, framing defaults and confirmation requirements are unchanged. Windows CI/QA must be evaluated on the new HEAD; no physical validation or merge.
