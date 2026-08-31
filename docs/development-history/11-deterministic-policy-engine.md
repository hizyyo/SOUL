# Deterministic Policy Engine

## Цель

Реализовать P0-движок политик (ULTRA_MVP §4.10, мастер-план §12.1–12.8): типизированный DSL SoulRule без динамического `eval`, без сетей, без регулярных выражений; эффекты `allow | deny | require_confirmation | redact` с решёткой deny > require_confirmation > redact > allow; оценка действия — чистая и детерминированная функция. Политики — база для Gateway-демо (SESSION-12, §4.11): «покупка на $600 → политика блокирует → коннектор не выполняется → локальная квитанция».

## Реализовано

### Rust: движок `src-tauri/src/policy.rs` (новый)

- **`SoulAction`** — структурированное действие (§12.8) с serde-camelCase: `action_id, kind, actor, connector_id, account_id, environment, recipient, domain, amount, currency, data_classes, reversible, confirmed_by_user, requested_scopes, payload_hash`.
- **`Atom`** — 7 типизированных операций (`eq, neq, in, lt, lte, gt, gte`), операнды `[Value; 2]` («путь поля», «литерал»). Whitelist полей `Field`/`FIELDS` (12 полей), префикс `action.` нормализуется. Валидация до записи: неизвестный путь отклоняется, числовые операции только на `amount`, `in` запрещён на boolean, типы литералов проверяются, `MAX_AMOUNT = 1e12`.
- **`Condition`** — `all` / `any` (непустые) / `not` / атом.
- **`SoulRule`** — `id, priority (0..=10000), when, effect, message (≤500)`; лимиты `MAX_RULE_JSON_CHARS = 4096`, `MAX_POLICY_RULES = 500`, `MAX_RULE_ID_CHARS = 128`.
- **`Decision`** — `effect, rule_id, message`; `evaluate()` — чистая функция: лучший по `priority DESC`, при равенстве — сильнейший эффект по `rank()`; сломанные строки пропускаются; нет совпадения → `allow`.
- **Хранилище**: таблица `policies` (id PK, priority с CHECK 0..10000, enabled, rule_json, created_at, updated_at) + `policy_meta`; `init_policies` вызывается из `init_db`; `seed_default_policies` — 2 демо-правила один раз за жизнь хранилища (флаг `seeded` в `policy_meta`; удалённые правила не воскресают, после `wipe_all` флаг сбрасывается — дефолты сеются заново).
- **CRUD**: `create_policy` (валидация до записи, UNIQUE-ошибка на повторный id), `list_policies` (priority DESC, id ASC), `set_policy_enabled`, `delete_policy`.
- **22 теста**: валидация (неизвестный путь/операция, не-finite сумма, неверный тип литерала, пустые комбинаторы, числовая операция на не-amount, priority/эффект, лимиты), семантика (gt по amount, kind+domain, recipient+sensitivity, reversible+confirmed, environment `in`, вложенные all/any/not, приоритеты, deny сильнее allow при равном приоритете, нет совпадения → allow, disabled пропускается, битое правило пропускается), БД (crud + seed-один-раз, повторный id отклонён, wipe очищает policies и seed-флаг).

### Rust: интеграция (`src-tauri/src/db.rs`, `src-tauri/src/lib.rs`)

- `db.rs`: `crate::policy::init_policies(&conn)?` в `init_db`; `wipe_all` удаляет `policies` и `policy_meta`.
- `lib.rs`: `mod policy;` + 5 Tauri-команд: `create_policy_cmd(rule_json)`, `list_policies_cmd()`, `set_policy_enabled_cmd(policy_id, enabled)`, `delete_policy_cmd(policy_id)`, `evaluate_action_cmd(action_json)`.

### TypeScript: модель `src/data/policy.ts` (новый)

- Зеркало Rust: `Effect`/`EFFECT_RANK` (решётка §12.3), `PolicyRow`, `Decision`, `SoulAction`, лимиты.
- `POLICY_PRESETS` — 4 детерминированных пресета (2 зеркалят сид: high-value → require_confirmation, destructive без подтверждения → deny; плюс recipient/domain deny и production/staging redact).
- `validateRuleJson` — лёгкая пред-проверка формы (JSON, id, integer-приоритет 0..10000, известный effect, лимиты символов); авторитет — Rust.
- `effectOfRuleJson`, `effectLabel`, `EVALUATION_EXAMPLE` (покупка $600 в production для playground).

### UI: страница Policies (`src/pages/Policies.tsx`, новый)

- Создание правила: выбор пресета (с описанием) + textarea JSON + Create; ошибки/успех инлайн.
- Список правил: id, бейдж эффекта, priority, updated, checkbox active (toggle), Delete; seed-семантика объяснена в подписи.
- Playground оценки (§4.11): textarea действия + Evaluate → Decision (эффект, правило, сообщение).
- Вкладка `Policies` в Nav + `App.tsx`.

### TypeScript-тесты `tests/policy.test.ts` (15)

- Решётка эффектов: deny > require_confirmation > redact > allow; лейблы на русском.
- Пресеты: оба сид-правила есть; каждый пресет — валидный SoulRule JSON с совпадающим id; неизвестный id → undefined; high-value содержит all[kind, amount>500].
- `validateRuleJson`: валидное правило; битый JSON; не-объект; пустой/отсутствующий id; неизвестный effect; приоритет 100.5/-1/10001 отклоняются, 10000 принимается.
- `effectOfRuleJson`: извлечение из строки строки; битый JSON/missing → null.
- `EVALUATION_EXAMPLE` парсится, kind=purchase.create, amount=600, environment=production.

## Изменённые файлы

- `src-tauri/src/policy.rs` — новый (DSL, хранилище, оценка, 22 теста).
- `src-tauri/src/db.rs` — `init_policies` в `init_db`; `wipe_all` + `policies`/`policy_meta`.
- `src-tauri/src/lib.rs` — `mod policy;` + 5 команд.
- `src/data/policy.ts` — новый (модель, пресеты, пред-валидация, пример действия).
- `src/pages/Policies.tsx` — новый (форма, список, playground).
- `src/components/Nav.tsx` — вкладка Policies.
- `src/App.tsx` — рендер `<Policies />`.
- `tests/policy.test.ts` — новый (15 тестов).
- `docs/development-history/11-deterministic-policy-engine.md` — этот файл.

## Изменённые контракты

- Новый контракт: **policy-engine/1** — таблицы `policies`/`policy_meta`; формат SoulRule (§12.5); семантика: best по priority DESC, при равенстве сильнейший эффект; сломанная строка пропускается; нет совпадения → `allow`; seed один раз за жизнь хранилища.
- Локальный протокол UI (не сетевой): `create_policy_cmd(rule_json)` → `PolicyRow`; `list_policies_cmd()` → `PolicyRow[]`; `set_policy_enabled_cmd(policy_id, enabled)` → `PolicyRow`; `delete_policy_cmd(policy_id)`; `evaluate_action_cmd(action_json)` → `Decision`.
- Схема SQLite: + таблицы `policies`, `policy_meta` (CREATE TABLE IF NOT EXISTS, миграций не требуется).

## Запущенные тесты

- `cargo test --lib`: **PASS** — 162 passed (включая 22 новых policy + регресс wipe).
- `cargo clippy --all-targets`: **PASS** — без предупреждений (почищены visibility `pub(crate)` для `Atom`/`Condition` и `#[cfg(test)]` для `Effect::as_str`).
- `cargo fmt --check`: **PASS**.
- `pnpm test`: **PASS** — 209 passed (включая 15 новых policy).
- `pnpm typecheck`: **PASS**.
- `pnpm lint`: **PASS**.
- `pnpm format`: **PASS**.
- `pnpm build`: **PASS**.
- `cargo build` (бинарный таргет): **PASS** — приложение собирается целиком.

## Проверка безопасности

- **Запрещённые возможности отсутствуют** (§12.7): нет динамического eval, нет shell/JS, нет сетевых вызовов со стороны политики, нет регулярных выражений, оценка не имеет побочных эффектов (чистая функция над read-only соединением).
- **Валидация до записи**: правило проверяется целиком (пути, типы, комбинаторы, приоритет, эффект, лимиты) до INSERT; битые строки не ломают оценку — пропускаются.
- **Защита размера**: лимиты на JSON правила (4 КБ), сообщение (500), id (128), приоритет (0..10000), число правил (500), сумма (1e12).
- **SQL** — только параметризованные запросы; `delete`/`update` идут по PK.
- **Детерминизм**: оценка зависит только от порядка строк (priority DESC, id ASC — стабильный порядок) и самого действия; никакой случайности, часов или сети.
- **Seed-семантика**: демо-правила не воскресают после удаления; после полного wipe (с очисткой ключей, SESSION-03) хранилище пересоздаётся с дефолтами заново.

## Влияние на производительность и токены

- Оценка — O(R) над правилами (R ≤ 500) без выделений, вне модели: 0 токенов, 0 сети.
- Валидация/лимиты на запись — однократные, O(размер JSON).

## Известные ограничения

- P0-множество полей/операций фиксировано (нет времени, freshness пользователя, регэкспов — §12.6 за пределами P0).
- Правила вводятся как JSON (UI-пресеты уменьшают боль; визуальный конструктор условий — P1).
- `evaluate_action_cmd` принимает действие целиком; интеграция с реальными точками применения (§12.2) — начиная с Gateway SESSION-12 (4.11) и коннекторов.
- Playground — песочница: он показывает решение политики, но никакой коннектор не выполняется (метка «Имитация: внешнее действие не выполнялось» — на странице Gateway в SESSION-12).

## Примечания по реализации

- Оператор `in` не десериализуется через `#[serde(rename = "in")]` внутри `#[serde(untagged)]` enum-а («data did not match any variant of untagged enum Atom»). Воспроизведено в изоляции (`serde-test` в temp-каталоге). Рабочее решение — raw-идентификатор поля `r#in`; `#[serde(rename)]` для untagged-вариантов не срабатывает. Паттерн-матчинг: `Atom::In { r#in } => r#in`.
- **Dev-среда**: с Node ≥17 на Windows vite с `host: false` биндится только на IPv6 `::1`, а Tauri опрашивает `localhost` по IPv4 — окно не открывалось («Waiting for your frontend dev server»). Фикс: `host: host || '127.0.0.1'` в `vite.config.ts`.

## Коммит

- `e4fe160` `feat(soul): deterministic policy DSL engine with policies UI tab [session-11]` (включает недокоммиченный `default-run = "soul"` из прошлой сессии)
