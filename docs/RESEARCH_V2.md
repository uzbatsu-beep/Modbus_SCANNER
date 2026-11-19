# v2.0 Platform research

[Русская версия](RESEARCH_V2.ru.md) · [Current roadmap](ROADMAP.md) ·
[Original v0/v1 research](RESEARCH.md)

**Date:** 2026-11-19 · **Scope:** protocol gateways, web companion, OT security
policies, plugin architecture · **Method:** official standards bodies, GitHub,
vendor documentation, industry blogs and community discussions via search.

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

## 8. Sources

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
