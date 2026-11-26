# Workbench Shell & Design System (PR #16)

Approved 2026-11-26: B, shell before connection manager, resizable/collapsible panels, compact engineering density, persistent layout, existing Dark/Light. Current version unchanged. No Rust or protocol changes.

## First shell
- Two-level header: native menu groups File/Project/View/Tools/Help with links to current pages, command bar with route navigation, register/monitor shortcuts, explicit scanner pause/resume/stop and monitor stop. Project file and About operations remain in existing Settings. No simulated connect/start action or new backend session.
- Left explorer groups actual discoveries by full transport configuration excluding Unit ID. This is a display grouping, not a connection ownership object. Search, expand/collapse, select and open registers. Right inspector summarizes current selected device, register/tag counts and existing monitor values. Full register editor stays in DeviceDetail.
- Results tabs separate devices/IP targets, register map and monitor. Existing monitor component remains mounted using v-show; ScanView and ResultsView cached across route changes. Other routes retain existing lifecycle. Single current device selection and one monitor session are unchanged.
- Bottom dock: latest 100 scanner events and traffic records, logger status, full traffic view link; no fabricated wire traffic or new recording. Status bar reads actual application version from IPC and active scan/monitor metrics.
- CSS variables for compact density and theme-dependent surfaces; existing theme preference persists through existing settings store. Layout only is saved separately in localStorage under modbus-scanner.workbench.layout.v1; malformed/unknown version resets to defaults. Reset and visibility controls always reachable. Default sizes L220/R250/B180 px; limits L160–480/R180–520/B100–420. Effective widths/height shrink or collapse at narrow sizes without overwriting saved preferences. Keyboard separators: arrows, Home/End; mouse drag.
- UI-03–UI-13 follow later PRs. No full docking framework, new dependencies, independent tabs for each device or migration of saved workspaces. No protocol change.

## Verification
Node unit tests for malformed layout, persistence round-trip, clamping, narrow window center, device grouping. Existing identity/measurement and Rust regressions remain required. Manual desktop acceptance (not automated by these tests): 1024x640 and larger; resize with mouse/keyboard, hide/restore/reset, restart, theme, switch away from monitoring and back, verify scan and monitor stay active, select devices, inspect logs and existing exports. Check narrow tables for horizontal scroll; verify UI on Windows QA build.

## RU
Оболочка Workbench реализуется поверх существующих функций. Панели изменяемые и сворачиваемые, настройки компоновки локальные. Дерево показывает только обнаруженные устройства; независимые соединения появятся в PR #17. Инспектор пока отображает сводные свойства устройства, полное редактирование регистров остаётся в рабочей области. Никаких новых сеансов и транспортных изменений. Обязательна визуальная проверка на Windows; автоматические тесты модели не заменяют её.
