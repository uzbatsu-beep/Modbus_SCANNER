# Modbus Engineering Workbench — vision / Концепция

**Decision 2026-11-26:** approved product direction for prototype development. This is a target, not a claim of delivered features. Name of repository/application remains Modbus Scanner.

## Цель
Локальный инженерный инструмент полного цикла для АСУ ТП: обнаружить устройство, исследовать карту регистров, интерпретировать данные, сохранить профиль, наблюдать технологические параметры, диагностировать обмен, воспроизвести поведение на виртуальном стенде, передать карту в систему автоматизации. Ориентир — ПЛК, ПЧ, счётчики, терморегуляторы, удалённые I/O, шлюзы, роботы с Modbus-интерфейсом.

## Продуктовые принципы
1. Достоверность инженерных данных важнее количества функций.
2. Пользователь управляет режимом опроса, периодом, группировкой и подключениями; приложение показывает фактический результат.
3. Один проект — несколько устройств и независимых транспортов, общий контекст данных; не несколько разрозненных окон.
4. Профиль связывает raw данные, документацию, единицы, состояния, версии прошивки и доказательства проверки.
5. Наблюдение и диагностика должны оставаться удобными на обычном инженерном ПК; работа локально и офлайн.
6. Сохранить текущие базовые ограничения OT, read-only discovery, подтверждение записи и безопасную остановку. RBAC, корпоративная авторизация, формальный security audit, облако и сложные политики — поздние этапы; не блокировать прототип новой административной инфраструктурой.

## Отличительные особенности
- Карта регистров как инженерный артефакт: FC-aware, семантические типы, гипотезы с доказательствами.
- Многоприборные тренды с точным декодированием и качеством данных.
- Диагностика связи с RTT, jitter, пропусками, статистикой запросов, raw кадрами и измеримым эффектом группировки.
- Профиль → эмулятор → воспроизводимый тест → экспорт карты в PLC/HMI/SCADA.
- В будущем: анализ захваченного обмена, проверяемые подсказки по документации, локальные адаптеры интеграции.

## Non-goals / Не цели ближайших версий
Full SCADA, continuously running production gateway, generalized network pentest, cloud accounts, enterprise RBAC, generic multi-protocol platform, autonomous AI writes. Preserve the in-house Rust Modbus protocol implementation.

## Success
An engineer can complete discovery-to-verified-tag workflow on an isolated bench, distinguish a bad value from a communications fault, reproduce the result without hardware, and export a usable map. See [ROADMAP](ROADMAP.md), [BACKLOG](BACKLOG.md), [UX design](UX_DESIGN.md).
