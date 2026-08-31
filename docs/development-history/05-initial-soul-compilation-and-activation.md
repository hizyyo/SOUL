# Initial SOUL Compilation and Activation

**Дата:** 2026-07-31

## Цель

Детерминированно превратить результаты калибровки в типизированные сущности (preference, decision, boundary, goal, fact) и подготовить безопасную активацию начального SOUL: компактный preview с возможностью изменить или исключить пункт, активация только после явного подтверждения, без единого вызова LLM на этом пути.

## Реализовано

- **Детерминированный компилятор `src/data/compile.ts`**: `compileAnswers` — чистая синхронная функция без сети, модели, времени и случайности. Каждый ответ калибровки → `CompiledItem` с типом строго из P0-набора (неизвестные question id и не-P0 категории пропускаются), данными `EntityData` (ID источника = questionId, область, уверенность, чувствительность, explicitness) и флагом `disputed`.
- **Правило спора (детерминированное)**: если в `bound_2` выбран «Nothing is off-limits», но в `bound_1` выбраны темы, которые ИИ не должен решать сам, оба результата помечаются спорными (`disputed: true`) и исключаются из массовой активации.
- **Идемпотентность повторной компиляции**: колонка `dedup_key` в `entities` + уникальный частичный индекс `(soul_id, dedup_key)`. Ключ = sha256(questionId + канонический value) для данных с `source: "calibration"`. Повторное добавление того же ответа возвращает существующую сущность без новой записи и без события — повторная компиляция одинаковых ответов идемпотентна, включая восстановление после сбоя.
- **Валидация типов P0 на границе Rust**: `add_entity` отклоняет любые типы вне {preference, decision, boundary, goal, fact}.
- **Preview начального SOUL (`src/pages/Preview.tsx`)**: компактный список кандидатов (тип, claim с маскировкой, уверенность, чувствительность, область, источник), чекбокс включения в активацию (заблокирован и выключен для границ/чувствительных/спорных — «requires individual confirmation in Inbox»), редактирование claim на месте, счётчики «будет активировано» и «требуют индивидуального подтверждения».
- **Безопасная активация (двухшаговая, оба шага проверяются на Rust)**:
  - `confirm_soul_preview` (команда `confirm_preview_cmd`): фиксирует явное подтверждение preview (`preview_confirmed=1`) и пишет событие `soul.preview_confirmed` (идемпотентно — повторный вызов без дублирующего события);
  - `activate_preview` (команда `activate_preview_cmd`): требует `preview_confirmed`, отклоняет уже активированный SOUL, валидирует каждый ID (сущность существует, принадлежит этому SOUL, статус candidate или уже active), fail-closed отклоняет границы/чувствительные/рискованные/спорные/отклонённые пункты, атомарно в транзакции активирует кандидатов (событие `entity.activated` на каждого) и пишет событие `soul.activated` с `previewConfirmed: true` и списком активированных ID.
  - Старый `activate_soul` (команда `activate_soul_cmd`) теперь тоже gated: без `preview_confirmed` отклоняет активацию; также пишет событие `soul.activated` (раньше события не было).
- **Поток приложения**: после завершения калибровки — переход на Preview (а не в Inbox); на Home кнопка «Review & Activate» ведёт на Preview вместо прямого вызова активации; `activate_soul_cmd` из UI больше не вызывается напрямую.
- **SoulInfo** расширен полем `preview_confirmed` (состояние переживает перезапуск).
- **Миграция БД**: аддитивная и идемпотентная — `ALTER TABLE ... ADD COLUMN` с игнорированием «duplicate column name» (стратегия документирована в коде и покрыта тестом миграции со старой схемой).
- **Inbox**: общий helper `requiresExplicitConfirm` (теперь покрывает также `restricted` и `disputed`).
- **`buildEntityData`**: text/writing-ответы теперь содержат `value` (= claim) — это делает dedup-ключ стабильным для текстовых ответов.

## Изменённые файлы

- `src-tauri/src/db.rs`: миграция (`dedup_key`, `preview_confirmed`, `add_column_if_missing`), валидация P0-типов, `dedup_key_for`/`find_by_dedup_key`, идемпотентный `add_entity`, `confirm_soul_preview`, gated `activate_soul`, `activate_preview`, `eligible_for_bulk_activation`, `get_soul_state` (4-кортеж), 15 новых тестов.
- `src-tauri/src/lib.rs`: `SoulInfo.preview_confirmed`, команды `confirm_preview_cmd`, `activate_preview_cmd`, `activate_soul_cmd` теперь принимает `device_id`.
- `src-tauri/src/package.rs`: тестовый seed вызывает `confirm_soul_preview` перед `activate_soul`.
- `src/data/compile.ts`: новый — детерминированный компилятор и правило спора.
- `src/data/review.ts`: `value` для text/writing, `disputed?: boolean`, helper `requiresExplicitConfirm`.
- `src/pages/Preview.tsx`: новый — компактный preview с включением/исключением, редактированием и двухшаговой активацией.
- `src/App.tsx`: компиляция через `compileAnswers`, переход на Preview после калибровки, обработчики подтверждения/активации preview.
- `src/pages/Home.tsx`: CTA «Review & Activate» → Preview (проп `onActivate` заменён на `onGoToPreview`).
- `src/pages/Inbox.tsx`: общий helper `requiresExplicitConfirm`.
- `src/components/Nav.tsx`: тип вкладки `preview` (без кнопки в навигации) + чистое форматирование prettier (файл больше не падает в `pnpm format`).
- `packages/soul-schema/src/event.ts`: операции `soul.preview_confirmed`, `soul.activated` в контракте событий.
- `tests/compile.test.ts`: новый — 11 тестов; `tests/review.test.ts`: +6 тестов.

## Изменённые контракты

- Схема БД: `entities.dedup_key` (TEXT, NULL для не-калибровочных), `soul_state.preview_confirmed` (INTEGER NOT NULL DEFAULT 0); уникальный индекс `idx_entities_dedup (soul_id, dedup_key)`.
- IPC: новая команда `confirm_preview_cmd(soul_id, device_id)`; новая команда `activate_preview_cmd(soul_id, entity_ids, device_id)`; `activate_soul_cmd` теперь принимает `device_id` и требует подтверждённого preview.
- `SoulInfo` + поле `preview_confirmed`.
- События: новые операции `soul.preview_confirmed`, `soul.activated` (обновлён `EventOperation` в soul-schema).
- `EntityData` + `value` для text/writing, опциональный `disputed`.
- Семантика: `activate_soul` больше не может активировать SOUL без явного подтверждения preview (fail-closed).
- Дублирование: повторный `add_entity` того же калибровочного ответа идемпотентен (dedup_key).

## Запущенные тесты

- `cargo test`: PASS — 46/46 (15 новых: валидация P0-типов, идемпотентность dedup (3 варианта), миграция старой схемы, идемпотентность подтверждения preview, gated-активация, активация preview: границы/чувствительные/риск/чужой/отклонённый/уже активный/пустой список/без подтверждения/повторная активация).
- `cargo clippy --all-targets`: PASS, без предупреждений.
- `pnpm typecheck`: PASS.
- `pnpm lint`: PASS.
- `pnpm test`: PASS — 59/59 (11 новых compile + 6 новых review).
- `npx prettier --check src/`: PASS для всех изменённых файлов; единственный предсуществующий сбой — `src/pages/Settings.tsx` (не менялся; Nav.tsx вылечен).
- `pnpm build`: PASS.

## Проверка безопасности

- **Угрозы**: массовое подтверждение чувствительных границ, активация без явного согласия, активация чужих/несуществующих/отклонённых сущностей, повторная активация, дублирование сущностей при повторной компиляции, неизвестные типы сущностей, некорректные данные.
- **Меры**:
  - `activate_preview` и `activate_soul` требуют серверно зафиксированного `preview_confirmed` (fail-closed: отсутствие → отказ, ничего не меняется).
  - Массовая активация отклоняет границы, чувствительные (`sensitive`/`restricted`), рискованные (`risk: true`), спорные (`disputed: true`) и отклонённые сущности; ошибка парсинга данных → тоже отказ.
  - Каждая сущность проверяется на принадлежность SOUL; неизвестный ID → отказ всей транзакции (ничего частично не активируется).
  - Повторная активация отклоняется; повторное подтверждение preview идемпотентно (без дублирующих событий).
  - `add_entity` валидирует тип P0, статус и JSON (объект, лимит claim) на границе Rust.
  - Все новые SQL-запросы параметризованы; ALTER TABLE использует только хардкод-имена (без пользовательского ввода).
  - Preview отображает claim через существующую маскировку и экранирование React; XSS-данные отображаются как текст.
  - События активации не содержат исходных данных сущностей (только ID).

## Влияние на производительность и токены

- Путь компиляции полностью детерминированный и синхронный: ноль вызовов модели, ноль токенов (тест `is idempotent and fully deterministic (zero tokens on this path)` гоняет 100 повторов с одинаковым выводом).
- Dedup: один индексный SELECT на сущность при повторной компиляции — O(1) на ключ.
- Активация — одна транзакция с N простых UPDATE/INSERT; p95 активации не измерялся отдельно, на реальных объёмах (≤25 сущностей) — мгновенно.
- Прирост записи: +2 события при активации (preview подтверждается как часть потока, событие `soul.preview_confirmed` при подтверждении + `soul.activated`).

## Известные ограничения

- Выбор включённых пунктов в preview — локальное состояние страницы: при перезаходе сбрасывается к умолчанию (все нечувствительные включены). Исключённые пункты остаются кандидатами в Inbox и могут быть активированы там по одному.
- Спорная комбинация детектируется только по правилу bound_2 «Nothing is off-limits» + bound_1 — это осознанный минимум без LLM-суждений.
- Маскировка в preview — UI-уровень; в БД данные хранятся в открытом виде (запланировано на поздние сессии).

## Повторная проверка и исправленные ограничения (fix-pass)

По результатам отчёта исправлены два ограничения финальной версии:

- **Отмена подтверждения preview**: добавлен `reset_soul_preview` (Rust, fail-closed: только до активации, идемпотентно, событие `soul.preview_revoked`), команда `reset_preview_cmd` и кнопка «Undo confirmation» на Preview. После отмены активация снова требует явного подтверждения (тесты: `reset_preview_is_idempotent_and_writes_revoked_event_once`, `reset_preview_blocks_activation_until_reconfirm`, `reset_preview_after_activation_is_rejected`).
- **Legacy-dedup**: `dedup_key_for` теперь даёт fallback-ключ по claim, когда `questionId`/`value` отсутствуют (данные до SESSION-05) — повторное создание таких сущностей идемпотентно (тест `add_entity_dedup_falls_back_to_claim_for_legacy_data`).

Проверки после фиксов: `cargo test` 50/50, `cargo clippy --all-targets` чисто, `pnpm test` 59/59, typecheck/lint/prettier/build — PASS.

## Последующие сессии

- По плану: контрольный центр состояний и поиск/компилятор контекста.

## Коммит

- `4920182` — `feat(soul): deterministic calibration compile with safe preview activation [session-05]`
- `80c9afa` — `fix(soul): preview confirmation reset and legacy dedup fallback [session-05]`
