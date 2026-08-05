# SESSION-16 - Полировка P0 и демонстрационный режим

## Цель

Сделать P0 понятнее и безопаснее для демонстрации без перехода к P1, оплате или production-заявлениям.

## Сделано

- Добавлен read-only demo-режим: откройте приложение с `?demo=1`. Он использует только статические синтетические данные, не вызывает Tauri IPC, не читает localStorage и не запускает внешние действия.
- Demo содержит видимый 55-секундный сценарий инвестора, включая честную финальную границу: P1 и оплата заблокированы до внешней P0-валидации.
- Навигация получила tablist-семантику, visible focus ring и управление `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- Добавлены адаптивный shell и nav для узких окон.
- Сбои инициализации больше не маскируются как отсутствие SOUL. Вместо сырых backend-ошибок основные экраны показывают безопасное сообщение с ID корреляции.
- Restore теперь начинается с privacy disclosure: файл и пароль обрабатываются локально, проверяются до замены данных, а замена требует отдельного подтверждения.
- Поиск Context теперь честно показывает ошибку, а начальная загрузка объясняет, что происходит локально.
- Подготовлены материалы внешней проверки: `P0_RECRUITMENT_FORM.md`, `P0_RESULTS_TEMPLATE.csv`, `P0_INVITATION_TEMPLATES.md`.

## Проверки

- `pnpm test`: PASS - 240 тестов.
- `pnpm exec tsc --noEmit`: PASS.
- `pnpm lint`: PASS.
- `pnpm format`: PASS.
- `pnpm build`: PASS.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`: PASS - 229 passed, 3 release-only benchmarks ignored в debug.
- `pnpm release:check`: PASS - policy p95 **9.8 µs**, cold context p95 на 1 000 сущностей **8.2093 ms**, cached context p95 на 10 000 сущностей **8.3 µs**.

## Ограничения

- Playwright flow не добавлен: попытка установить `@playwright/test` 5 августа 2026 года остановилась из-за недоступного npm registry (`EAI_AGAIN` / `ENOTFOUND`). Проект не содержит сломанный dependency; demo-fixtures, сценарий, safe errors и клавиатурная навигация покрыты unit-тестами.
- Реальная P0-валидация по-прежнему `BLOCKED`: шаблоны не являются результатами и не заменяют внешних участников, day-7/day-28 retention и решение основателя.
