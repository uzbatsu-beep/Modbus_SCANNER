# Market & community research

[Русская версия](RESEARCH.ru.md) · [Development plan](PLAN.md)

**Research date:** 2026-11-18. **Method:** publicly available sources — official
Modbus Organization site, docs of commercial and open-source tools, GitHub repos,
academic publications (IEEE, Springer, MDPI, Computers & Security), forums/Reddit
(via search snippets), industry guides and standards (ISA/IEC 62443, NIS2).

> Limitation: the built-in `websearch` integration was unavailable and some sites
> (Reddit, ScienceDirect, PDF specs) blocked direct access. Search snippets and
> alternative mirrors were used where needed; this is noted in the text.

---

## 1. Official sources

Source: [Modbus Organization — Specifications and Implementation Guides](https://modbus.org/modbus-specifications)

| Document | Relevance to the product |
|----------|--------------------------|
| Modbus Application Protocol V1.1b3 | Base set of FCs, addressing, exceptions — our protocol layer already follows it |
| Modbus Serial Line Protocol and Implementation Guide V1.02 | Recommended for new implementations (our RTU/ASCII codec follows it) |
| Modbus Messaging Implementation Guide v1.0b (TCP/IP) | MBAP, client/server/gateway, object model |
| **Modbus Security Protocol (TLS, port 802)** | Future vector: authentication and integrity via TLS + X.509v3; not implemented by us or by most scanners yet |

Conclusion: the protocol foundation is correct; the closest official growth
vectors are **Modbus TLS/TCP 802** and identification/diagnostic function codes.

---

## 2. What field tools do

### 2.1 Modbus Logic Scanner — the read-only reference
Source: [docs.modbuslogic.com](https://www.modbuslogic.com/docs/modbus-scanner/)

Capabilities that set the bar for our product:

- **Discover**: CIDR/IP range, port, timeout, concurrency; classification into
  `valid Modbus response` / `port open but no Modbus` / `refused` / `timeout` /
  `unreachable`. An open port 502 alone is **never** reported as a device.
- RTU: unit-ID sweep on one port; for TCP — unit-ID discovery behind a gateway
  (1–10 by default).
- **Register scan** with three strategies: `Quick` (blocks, failed block → 1/5),
  `Balanced` (100 → 20 → 5, then edge refinement), `Deep` (down to single
  registers). Timeouts are never split (too expensive); 3 consecutive timeouts
  abort; exception 01 aborts immediately.
- Exception `02 Illegal Data Address` is treated as **normal**: the device is
  alive, the address just does not exist (also described in the [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan)).
- Addresses shown in 4 notations: zero-based, one-based, reference, hex.
- Decoding: INT16/UINT16/INT32/UINT32/FLOAT32/ASCII + **all byte orders**;
  hints like “possible float32 (ABCD)” always labelled as a guess, never a fact.
- Named registers: name, type, byte order, **scaling** `(raw × mul ÷ div) + offset`,
  units, description → `io-tags.json`.
- **Live Monitor**: polling 250 ms–10 s, min/max, trend 1 min–1 h; monitoring is
  the final check that a decoding is correct.
- Exports: `io-tags.json`, CSV/Excel, raw register CSV, simulator CSV; projects
  with scan history.
- **Read-only by design**: only FC01–04 and read-only FC43/14; integration tests
  assert the mock device receives **zero** write requests. Nothing runs in the
  background, loopback only, no telemetry.
- Explicit warning: even reads add load to industrial devices.

### 2.2 Modbus Connect — monitoring and logging
Source: [modbusconnect.com/features](https://modbusconnect.com/features)

- Discovery: CIDR, **quiet mode for production** and fast mode with concurrency,
  IDs 1–247, extraction of **vendor/product code/model**, bulk add.
- Live monitoring: change highlighting, custom labels, dec/hex/bin, session
  pause/resume without data loss, 1-based offset support.
- Charts with up to 12 series, zoom/pan, pop-out windows, CSV export of chart data.
- **Protocol-level debugging**: raw TX/RX hex with timestamps, FC and errors.
- File logging: CSV/JSON/TAB, rotation daily/session/size, multiple loggers.

### 2.3 Other tools (search-summary)

| Tool | Differentiator | Source |
|------|----------------|--------|
| CAS Modbus Scanner | Binary/HEX/Uint16/Int16/Uint32/Int32/Float32; device search without a known address | [Chipkin](https://store.chipkin.com/products/tools/cas-modbus-explorer) |
| modbus-rtu-tool (GitHub) | Full bus scan: **all baud/parity/stop + IDs 1–247**, register map scan, 10×N monitor, write panel, RU/EN i18n, single-file exe | [GitHub](https://github.com/olegchumakov/modbus-rtu-tool) |
| modbus-probe (GitHub) | Security audit: unit enum, FC 1–127 fuzz, write test with rollback, FC43 fingerprinting, **PCAP capture**, multithreading, JSON/CSV, severity, `--monitor`, `--discover` CIDR; rationale — IEC 62443 / NIS2 | [GitHub](https://github.com/isecwire/modbus-probe) |
| Nmap `modbus-discover` | Unit-ID enumeration + vendor/product read via FC2B/MEI0E | [Nmap NSE](https://nmap.org/nsedoc/scripts/modbus-discover.html) |
| ModbusManager / ModbusSeeker | Automatic baud/parity and slave-ID detection as the core value (“no documentation”) | [modbusmanager.com](https://modbusmanager.com/modbus-scanner.html), [modbusseeker.com](https://modbusseeker.com) |
| ModbusLens | Free TCP/RTU scanner + client + server, live tags, trends | [ModbusLens](https://craftparking.github.io/ModbusLens/) |
| modbus-map-maker | Turns register spreadsheets into validated endianness-aware specs + code | [GitHub](https://github.com/casalifrancesco98/modbus-map-maker) |
| modpoll / pymodbus / QModMaster / OpenModScan | CLI scripting, cross-platform, basic GUI | [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan) |

### 2.4 Community signals

- [Reddit r/PLC “Any helpful Modbus diagnostic tools out there?”](https://www.reddit.com/r/PLC/comments/f2728e/any_helpful_modbus_diagnostic_tools_out_there/):
  the snippet highlights the pain of “decoding the data”, **node address**, and
  **one-based vs zero-based addressing**. Users get stuck on interpreting addresses
  and data even after discovery.
- [Chipkin discovery guide](https://docs.chipkin.com/articles/modbus-device-discovery-and-network-scanning-guide/)
  and [ModbusCloud KB](https://www.modbuscloud.com/knowledge-base/modbus-network-scan):
  * sweep speed/framing first (9600 8N1 → 2400 8N1 → 19200 8N1 → 8E1), then addresses;
  * pitfalls: **duplicate addresses → CRC collisions**, broadcast 0, missing termination;
  * an RS-485 bus must have exactly one master — scanning while a PLC is polling
    produces unusable results;
  * a one-off snapshot does not replace continuous monitoring;
  * nmap: `-p 502 --open`, `modbus-discover`.
- Conclusion: “just find the address” is commodity. Value lies in **autodetection +
  correct interpretation + safety + export into real work**.

---

## 3. Academic research

| Work | Implication for the product |
|------|-----------------------------|
| [Passive Fingerprinting of ICS Devices Using Aggregates of Modbus Traffic (IEEE, 2025)](https://ieeexplore.ieee.org/document/11471618/) | “Active scanning may have adverse effects on an industrial network” → a **passive/benign mode** and minimal active probing are needed |
| [Remote field device fingerprinting using device-specific Modbus information (IEEE, 2017)](https://ieeexplore.ieee.org/document/7870006) | Device/model identification from Modbus responses is feasible |
| [On the Feasibility of Device Fingerprinting in ICS, LNCS 8328 (Springer, 2014)](https://link.springer.com/chapter/10.1007/978-3-319-03964-0_14) | Survey and guidelines for reliable ICS fingerprinting |
| [Development of Fingerprint Identification Based on Device Flow (MDPI Applied Sciences, 2023)](https://www.mdpi.com/2076-3417/13/2/731) | Traffic periodicity as a device feature — basis for a future passive mode |
| [MOSTO: toolkit for security auditing of ICS devices (Computers & Security, 2023)](https://www.sciencedirect.com/science/article/pii/S0167404823002833) | GPL auditing tool that does “not interfere with ICS workflows” — matches our read-only stance |
| [ANDVI: Automated Network Device and Vulnerability Identification](https://par.nsf.gov/servlets/purl/10495895) | Modbus ADU extraction from TCP packets for identification |
| [Read Device Identification FC43/MEI14](https://www.merobix.com/blog/what-is-modbus-function-code-43-device-identification) | Basic ID: VendorName, ProductCode, Revision — optional part of the spec |

Academic consensus: active OT scanning can be harmful; a safe read-only scanner
with a quiet mode and passive fingerprinting is not just UX — it is a competitive
advantage and alignment with good practice.

---

## 4. Standards and regulation

- [ISA/IEC 62443 series](https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards) —
  zones/conduits, SL1–SL4; requires access control that Modbus lacks.
- EU **NIS2** and **Cyber Resilience Act** (vulnerability reporting obligation
  from June 2026, see [ModulesLink](https://www.moduleslink.com/article-iec62443-industrial-cybersecurity.html)) —
  growing demand for OT auditing; a security mode is welcome but must be opt-in
  and authorized.
- Practical conclusion: **do not put write tests in the main flow**. If implemented,
  use a separate mode with confirmation, automatic rollback and a liability notice.

---

## 5. Demand ranking

| # | Capability | Frequency in sources | Our MVP |
|---|------------|----------------------|---------|
| 1 | Auto-detect baud/parity/stop + unit-ID sweep | Very high (almost every field guide) | Partial (IDs yes, speed sweep no) |
| 2 | Smart register-map scan (blocks/split/exception-aware) | Very high | Partial (linear `register_plan`) |
| 3 | Type/byte-order/scaling decoding, named tags | Very high | No (raw hex/dec/bin only) |
| 4 | Read-only by design, quiet mode, limits, authorization | High (community + standards) | Partial (read-only yes, quiet no) |
| 5 | FC43/14 device identification + vendor/product | High | No |
| 6 | Live monitoring, trends, change highlighting, log rotation | High | No |
| 7 | Tag-map/project/export/history/diff exports | Medium-high | Basic CSV/JSON/XML |
| 8 | IP discovery with “port open but not Modbus” classification, concurrency | Medium-high | Partial (sequential) |
| 9 | Raw TX/RX log, timings, PCAP, FC support map/fuzz | Medium (security niche) | No |
| 10 | CLI/headless/REST, scripting, integrations | Medium | No |
| 11 | Multi-protocol (MQTT/OPC UA/REST gateway) | Medium (adjacent market) | Out of scope |
| 12 | Localization, single-file distribution, hotkeys | Medium | Partial (RU/EN docs, mixed UI) |

---

## 6. Key insight

> The market for simple “address scanners” is saturated (modpoll, CAS, QModMaster,
> nmap). The gap is at the **discovery → understanding** boundary: automatic serial
> setup, safe register-map scanning, correct float/endianness decoding, device
> identification and export of a finished tag map. Our custom protocol stack gives
> control over timing and framing — this is our differentiator versus pymodbus
> wrappers.

---

## 7. Sources

1. https://modbus.org/modbus-specifications
2. https://www.modbuslogic.com/docs/modbus-scanner/
3. https://modbusconnect.com/features
4. https://www.modbuscloud.com/knowledge-base/modbus-network-scan
5. https://docs.chipkin.com/articles/modbus-device-discovery-and-network-scanning-guide/
6. https://github.com/olegchumakov/modbus-rtu-tool
7. https://github.com/isecwire/modbus-probe
8. https://www.reddit.com/r/PLC/comments/f2728e/any_helpful_modbus_diagnostic_tools_out_there/
9. https://ieeexplore.ieee.org/document/11471618/
10. https://ieeexplore.ieee.org/document/7870006
11. https://link.springer.com/chapter/10.1007/978-3-319-03964-0_14
12. https://www.mdpi.com/2076-3417/13/2/731
13. https://www.sciencedirect.com/science/article/pii/S0167404823002833
14. https://par.nsf.gov/servlets/purl/10495895
15. https://www.merobix.com/blog/what-is-modbus-function-code-43-device-identification
16. https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards
17. https://nmap.org/nsedoc/scripts/modbus-discover.html
18. https://github.com/black-a85/ModbusTool
19. https://craftparking.github.io/ModbusLens/
20. https://github.com/casalifrancesco98/modbus-map-maker
