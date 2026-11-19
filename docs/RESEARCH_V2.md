# v2.0 Platform research

[Русская версия](RESEARCH_V2.ru.md) · [Current roadmap](ROADMAP.md) ·
[Original v0/v1 research](RESEARCH.md)

**Date:** 2026-11-19 · **Scope:** protocol gateways, web companion, OT security
policies, plugin architecture · **Method:** official standards bodies, GitHub,
vendor documentation, industry blogs and community discussions via search.

**Addendum (§9):** live registry snapshot (crates.io API + GitHub search,
2026-11-19) backing the concrete library choices in the v2.0 backlog.

---

## 1. Market landscape / what gateways actually do

| Project / product | What it confirms |
|-------------------|------------------|
| [ThingsBoard IoT Gateway](https://github.com/thingsboard/thingsboard-gateway) (2.2k★, Apache-2.0) | Modbus, OPC UA, BACnet, MQTT, REST, ODBC, SNMP in one gateway; **data mapping/converters**, local buffering, auto-reconnect, remote configuration and remote logs from a web UI; custom Python connectors/converters |
| [Apache StreamPipes](https://streampipes.apache.org) | OPC UA, MQTT, S7, Modbus → visual pipelines, dashboards and datasets |
| [EMQX Neuron](https://hysenlabs.com/projects/emqx-neuron) (LGPL-3.0) | Modbus/OPC UA/MQTT gateway with web UI on port 7000 |
| [MultiBus Gateway](https://multibus.dev) | Modbus + MQTT + HTTP + OPC UA, virtual meters, **safe writes**, own hardware |
| [Modbus Cloud gateway comparison](https://www.modbuscloud.com/knowledge-base/industrial-iot-gateway-comparison) | 2026 buyer criteria: Modbus + BACnet + MQTT, NIS2/CRA cybersecurity, EU pricing |
| [HiveMQ: OPC UA ↔ MQTT bridge](https://www.hivemq.com/blog/opc-ua-mqtt-bridge-ot-protocols-industrial-data/) | Bridging OT protocols to scalable MQTT backbones is a mainstream architecture |

**Conclusion:** the differentiator is not "yet another Modbus client" — it is
**translation + mapping + buffering + remote management**, ideally with our
existing read-only safety model and local-first storage.

## 2. Standards that matter for v2.0

- **OPC UA** (IEC 62541, [OPC Foundation](https://opcfoundation.org/about/opc-technologies/opc-ua/)):
  object-oriented information model, 60+ companion specifications, client/server
  and Pub/Sub, built-in signed/encrypted sessions with X.509. A Modbus scanner's
  tag map can become an OPC UA address space with little semantic loss.
- **Eclipse Sparkplug B** ([specification v3.0](https://sparkplug.eclipse.org/specification/),
  [Tahu reference implementation](https://projects.eclipse.org/projects/iot.tahu)):
  defines MQTT topic namespace, Protobuf payload and session state (birth/death
  certificates). Plain MQTT moves messages; Sparkplug makes them interoperable
  with SCADA/HMI — important if we publish scan results to a broker.
- **BACnet / BACnet-IP** (ASHRAE 135-2024, [bacnet.org](https://bacnet.org)):
  object-oriented building automation standard (HVAC, lighting, metering),
  ISO 16484-5; BACnet/SC adds secure communication. Modbus→BACnet gateways are
  a distinct, high-demand building-automation niche.
- **Modbus Security** (port 802, TLS + mutual X.509) — implemented in E-05 and
  explicitly recommended by [RTA's 12 best practices](https://www.rtautomation.com/rtas-blog/securing-modbus-networks/).

## 3. OT security implications (IEC 62443)

Sources: [IEC 62443 zone/conduit guide](https://www.cagripolat.com/iec62443/en/iec-62443-zone-conduit-design-ot-practical-guide),
[MDPI on zones and conduits](https://www.mdpi.com/2624-800X/6/2/52),
[RTA 12 practices](https://www.rtautomation.com/rtas-blog/securing-modbus-networks/).

Practical rules we must encode as *policies*, not just documentation:

1. Isolate Modbus networks from IT/enterprise networks; connect through a
   controlled gateway (zone/conduit, SL-1…SL-3).
2. Allow only required function codes — monitoring needs FC01–FC04; block writes
   (FC05/06/15/16) unless explicitly authorized (NIST least privilege).
3. **No pass-through commands** — anything a client can send raw through a
   gateway is an attack surface. Our `send_raw_frame` is for debugging only and
   must be gated.
4. Remote access through MFA/VPN, time-bound; no direct remote access to devices.
5. Monitor for anomalies: new client IPs, unexpected FCs, abnormal register
   ranges, **rapid Unit ID scanning or abnormal polling frequency**. Our scanner
   intentionally does Unit ID sweeps — v2.0 must ship a "quiet" profile and
   authorization acknowledgment so it does not look like an attack.
6. Data diode for monitoring-only zones is the gold standard; our UDP/MQTT
   publish path can mirror that one-way flow logically.

## 4. Web companion

Gateways above expose web UIs for remote config, logs and dashboards. For our
project the natural evolution is **local-first**: reuse the v1.0 REST + WebSocket
API (`/api/v1/*`, `monitor:update`) as the backend for a small companion web
dashboard (project list, live trends, device cards). No cloud or telemetry by
default; explicit opt-in only for MQTT publish.

## 5. Plugin architecture

[Tauri plugins](https://v2.tauri.app/develop/plugins/) are compile-time,
dual-layer (Rust backend + typed JS API) — good for first-party extensions but
not for user-installed runtime plugins. Rust's `dyn Trait` ABI is not stable
across compiler versions, so runtime plugins need one of:

- versioned C ABI (`abi_stable`-style) — high maintenance burden;
- **WASM sandbox** (`wasmtime`/`extism`) — safer, portable, clear capability
  boundaries; recommended for community decoders/transports;
- keep protocol modules as in-tree feature crates with stable internal traits
  (`Transport`, `ScanStrategy`, `EventSink`) — our E-01 decoupling already
  created the right seams.

Recommendation: **compile-time modules first, WASM sandbox second**, no raw dylib
loading.

## 6. Prioritised v2.0 backlog (proposal)

| ID | Item | Impact | Effort | Risk | Depends on |
|----|------|--------|--------|------|------------|
| G-01 | Policy engine: zones/target allowlists, quiet mode, read-only enforcement, authorization acknowledgment | 9 | M | High | v1.0 core |
| G-02 | MQTT publisher (plain JSON + Sparkplug B protobuf) over local broker, opt-in | 8 | L | Medium | G-01 |
| G-03 | OPC UA server: expose tag map as address space, signed/encrypted sessions | 8 | L | Medium | v1.0 tags |
| G-04 | Web companion dashboard served from the local API | 7 | M | Low | v1.0 REST/WS |
| G-05 | BACnet/IP connector (read-only objects, COV subscription) | 6 | L | Medium | G-01 |
| G-06 | Plugin SDK: stable extension traits + WASM sandbox for decoders/transports | 6 | L | High | G-01, E-01 |

## 7. Risks / mitigations

| Risk | Mitigation |
|------|------------|
| Gateway becomes a pass-through attack vector | G-01 policy engine; raw frames disabled by default |
| MQTT to cloud leaks OT data | opt-in, loopback/local broker default, topic allowlist, no telemetry |
| OPC UA certificate management complexity | start with server mode + security policy None/Sign, document cert workflow |
| BACnet stack size | isolate behind a feature flag; consider mature crate |
| WASM plugin sandbox limits hardware access | plugins only transform data; transports stay in core |

## 9. Technology selection — addendum (2026-11-19)

The first pass answered *what* a v2.0 platform should do. This addendum answers
*what we build it with*, using live registry data so the backlog in
[ROADMAP.md](ROADMAP.md) rests on verifiable maintenance and licence facts.

### 9.1 Candidate libraries (snapshot 2026-11-19)

| Area | Crate / project | Version | Last release | Licence | Recent downloads | Verdict |
|------|-----------------|---------|--------------|---------|------------------|---------|
| OPC UA | [`async-opcua`](https://github.com/freeopcua/async-opcua) | 0.19.0 | 2026-07-18 | MPL-2.0 | 121k | **Chosen** — active async client+server, separate `server` feature |
| OPC UA (legacy) | [`opcua`](https://github.com/locka99/opcua) | 0.12.0 | 2024-01-01 | MPL-2.0 | 27k | Rejected — no release in ~2 years (repo pushed 2025-04) |
| MQTT | [`rumqttc`](https://github.com/bytebeamio/rumqtt) | 0.25.1 | 2025-11-21 | Apache-2.0 | 2.59M | **Chosen** — de-facto standard, rustls + WebSocket features |
| MQTT (alt) | [`paho-mqtt`](https://github.com/eclipse/paho.mqtt.rust) | 0.14.0 | 2026-03-26 | EPL-2.0 | 392k | Backup only — EPL-2.0 patent terms need review |
| BACnet | [`bacnet-rs`](https://github.com/bacnet-rs/bacnet-rs) | 0.3.2 | 2026-09-02 | MIT OR Apache-2.0 | 8k | **Watchlist** — maintained but pre-1.0, small community (~28★) |
| BACnet (legacy) | [`bacnet`](https://github.com/BiancoRoyal/bacnet-rust) | 0.1.0 | 2018-05-14 | MPL-2.0 | 23 | Rejected — abandoned |
| Sparkplug B | no mature Rust crate | — | — | — | — | **Own encoder** with [`prost`](https://github.com/tokio-rs/prost) 0.14.6; GitHub search found only 0–10★ prototypes |
| WASM plugins | [`extism`](https://github.com/extism/extism) | 1.41.0 | 2026-10-09 | BSD-3-Clause | 239k | **Chosen** for G-06 (capability-based SDK) |
| WASM runtime | [`wasmtime`](https://github.com/bytecodealliance/wasmtime) | 51.0.0-rc | 2026-11-19 | Apache-2.0 WITH LLVM-exception | 13.4M | **Chosen** as runtime, but pin a *stable* release, not an RC |
| Modbus (reference only) | [`tokio-modbus`](https://github.com/slowtec/tokio-modbus) | 0.17.0 | 2025-10-22 | MIT OR Apache-2.0 | 477k | Not used — the own-stack rule is unchanged |

### 9.2 Decisions mapped to the backlog

| ID | Decision | Rationale |
|----|----------|-----------|
| G-01 | Pure core logic, no new dependencies | Policy engine is logic, not I/O; must be testable without hardware |
| G-02 | `rumqttc` + `prost`; local broker, opt-in publish | No mature Sparkplug B Rust library: JSON path first, Protobuf second, own thin codec |
| G-03 | `async-opcua` in server mode, `server` feature only | Active upstream; start with None/Sign security, document the X.509 workflow |
| G-04 | Vue 3 + the existing REST/WS API; no new backend | Lowest risk, reuses the v1.0 IPC contract |
| G-05 | Defer; track `bacnet-rs` until 1.0 or a contributing community | Pre-1.0 stack in a safety-adjacent path is a supply-chain risk |
| G-06 | In-tree stable traits now (E-01 seams), `extism` + pinned `wasmtime` later | No raw dylib ABI; WASM capabilities match our data-transform-only plugin model |

### 9.3 Licence compatibility (GPL-3.0 application)

MPL-2.0 (`async-opcua`), Apache-2.0 (`rumqttc`, `wasmtime`, `prost`), MIT and
BSD-3-Clause (`extism`) are all GPL-3.0-compatible: none adds a restriction to
the combined work, and MPL-2.0 copyleft is file-level. The exception to watch is
EPL-2.0 (`paho-mqtt`) — its patent-retaliation and exclusion clauses need
explicit review before adoption, which is why `rumqttc` is preferred. Runtime
plugins must ship their own licence notices; the WASM sandbox is not a linking
exemption.

### 9.4 New risks discovered

| Risk | Mitigation |
|------|------------|
| OPC UA upstream is 0.x and may break its API between minors | Pin the exact version and isolate it behind our own adapter trait |
| `bacnet-rs` is 0.3.x with a small community | Keep G-05 behind a feature flag; revisit after 1.0 or implement a minimal read-only BACnet/IP subset ourselves |
| No mature Sparkplug B Rust implementation | Own thin Protobuf encoder validated against Tahu test vectors; never invent a custom payload |
| wasmtime publishes RCs frequently | Pin a stable release in `Cargo.lock`, bump deliberately, never track `*` |

### 9.5 Sources (addendum)

15. https://crates.io/api/v1/crates/async-opcua · https://github.com/freeopcua/async-opcua
16. https://crates.io/api/v1/crates/opcua
17. https://crates.io/api/v1/crates/rumqttc
18. https://crates.io/api/v1/crates/paho-mqtt
19. https://crates.io/api/v1/crates/bacnet-rs · https://github.com/bacnet-rs/bacnet-rs
20. https://crates.io/api/v1/crates/bacnet
21. https://api.github.com/search/repositories?q=sparkplug+rust · https://sparkplug.eclipse.org/specification/
22. https://crates.io/api/v1/crates/prost
23. https://crates.io/api/v1/crates/extism · https://crates.io/api/v1/crates/wasmtime
24. https://crates.io/api/v1/crates/tokio-modbus

---

## 10. Sources

1. https://github.com/thingsboard/thingsboard-gateway
2. https://streampipes.apache.org
3. https://multibus.dev
4. https://www.modbuscloud.com/knowledge-base/industrial-iot-gateway-comparison
5. https://www.hivemq.com/blog/opc-ua-mqtt-bridge-ot-protocols-industrial-data/
6. https://sparkplug.eclipse.org/specification/
7. https://projects.eclipse.org/projects/iot.tahu
8. https://opcfoundation.org/about/opc-technologies/opc-ua/
9. https://reference.opcfoundation.org/specs/OPC-10000-2/4.5
10. https://bacnet.org · https://data.ashrae.org/BACnet/
11. https://www.cagripolat.com/iec62443/en/iec-62443-zone-conduit-design-ot-practical-guide
12. https://www.mdpi.com/2624-800X/6/2/52
13. https://www.rtautomation.com/rtas-blog/securing-modbus-networks/
14. https://v2.tauri.app/develop/plugins/
