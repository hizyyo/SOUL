# SESSION-12 — Имитированный Gateway

## Цель

Реализовать P0-демонстрацию Gateway (ULTRA_MVP §4.11): честная локальная имитация внешнего действия через поддельный коннектор. Агент предлагает действие (`SoulAction`) → gateway нормализует его → оценивает движком политик (SESSION-11) → при разрешении выпускает локальную capability (action id, hash нагрузки, nonce, срок, однократное использование) → выполнение только через поддельный коннектор (без сети, без реальных внешних вызовов) → квитанция со статусом имитации. Интерфейс явно помечен: «Имитация: внешнее действие не выполнялось.» Реальная изоляция учётных данных и атомарное выполнение — P1; имитация P0 не выдаётся за защиту production-уровня.

## Реализовано

### Rust: модуль `src-tauri/src/gateway.rs` (новый)

- **Нормализация** `normalize_action(json) -> SoulAction`: парсинг, обрезка пробелов, обязательные поля (actionId/kind/actor/connectorId/accountId), диапазон суммы (±1e12), лимит JSON (16 КБ). Входной `payloadHash` агента не доверяется — hash нагрузки пересчитывается из нормализованного действия.
- **Hash нагрузки** `payload_hash_of(action)`: SHA-256 (hex) от канонического JSON действия без поля payload_hash (детерминированно, покрывает всю нагрузку).
- **Capability** (таблица `capabilities`): id `cap_<uuid>`, action_id, kind, payload_hash, nonce (`uuid` v4, UNIQUE), срок `expires_at` (RFC3339), однократность `used_at`, сохранённая нагрузка `action_json` (для повторной оценки политики; наружу не отдаётся — API показывает только hash). TTL: по умолчанию 300 с, диапазон 1..3600 с.
- **Поддельный коннектор** `fake_connector_execute(action)`: детерминированный результат (`sim_<hash>`) без сети и побочных эффектов — единственная точка «исполнения» действия в P0.
- **Этап предложения** `propose_action(conn, action_json, ttl)`: нормализация → `policy::evaluate` → deny/require_confirmation/redact → квитанция (denied/held/redacted) без capability; allow → capability + квитанция `pending`.
- **Этап выполнения** `execute_capability(conn, capability_id, connector, account, environment, action_json)`: порядок проверок — существование → не использована → не истекла → hash нагрузки совпал → канал в локальном реестре имитированных коннекторов → повторная оценка политики (изменение правил между предложением и выполнением тоже блокирует) → поддельный коннектор → capability помечается использованной, квитанция `pending` обновляется до `simulated`. Каждый отказ оставляет квитанцию `refused` с причиной (без обращения к коннектору); неудачная проверка не сжигает capability, сжигает только реальная имитация.
- **Реестр каналов** (таблица `gateway_connectors`, PK по тройке): 4 демо-строки, сеются один раз за жизнь хранилища (флаг в `gateway_meta`, как демо-политики SESSION-11). Проверка канала даёт детерминированную причину: `connector mismatch` / `account mismatch` / `environment mismatch`.
- **Квитанции** (таблица `gateway_receipts`): id, capability_id, action_id, kind, status (`pending|simulated|denied|held|redacted|refused`), decision_effect, rule_id, message, connector_executed, reason, nonce, created_at. Без исходной чувствительной нагрузки (как квитанции решений SESSION-11).
- **25 тестов**: нормализация (обрезка + детерминированный hash, пустые обязательные поля, лимиты, битый JSON), предложение (capability с nonce/сроком, уникальность nonce, deny сидом, require_confirmation сидом, redact кастомным правилом, кламп TTL, ошибка), выполнение (успех + однократность, повтор → отказ, изменённая нагрузка → отказ, неверный коннектор/учётка/окружение → отказ, истёкшая → отказ, неизвестная → отказ, политика на момент выполнения, запрещённое действие не достигает коннектора, детерминизм поддельного коннектора), хранилище (списки свежими первыми, сид реестра идемпотентен, wipe_all очищает и пересеивает).

### Rust: интеграция (`src-tauri/src/db.rs`, `src-tauri/src/lib.rs`)

- `db.rs`: `crate::gateway::init_gateway(&conn)?` в `init_db`; `wipe_all` удаляет `capabilities`, `gateway_receipts`, `gateway_connectors`, `gateway_meta` (реестр пересеивается после полной очистки).
- `lib.rs`: `mod gateway;` + 4 Tauri-команды: `gateway_propose_cmd(action_json, ttl_seconds)`, `gateway_execute_cmd(capability_id, connector_id, account_id, environment, action_json)`, `list_gateway_receipts_cmd()`, `list_gateway_capabilities_cmd()`.

### TypeScript: модель `src/data/gateway.ts` (новый)

- Зеркало Rust: `GatewayStatus` (6 статусов), `GatewayCapability`, `GatewayReceipt`, `GatewayProposal`, `GatewayExecuteResult`, константы TTL/лимитов.
- `SIMULATION_LABEL = 'Имитация: внешнее действие не выполнялось.'` — точная метка §4.11.
- `GATEWAY_CONNECTOR_OPTIONS` — зеркало сида реестра (4 канала) для UI-селекта; `channelLabel`.
- `GATEWAY_EXAMPLE_ACTION` — покупка на $600 в production (демо §4.11).
- `validateActionJson` — лёгкая пред-проверка формы (JSON, обязательные поля, лимит); авторитет — Rust.
- `capabilityState` (ready/used/expired), `shortDigest`, русские лейблы и тона статусов.

### UI: секция Gateway (`src/pages/GatewaySection.tsx`, новый) внутри вкладки Policies

- Открытое решение: новой вкладки нет (UX §3.1 — только основные разделы; «не плоди лишние вкладки»). Секция «Имитированный Gateway (§4.11)» рендерится на странице Policies под playground'ом оценки.
- Жёлтый баннер с точной меткой «Имитация: внешнее действие не выполнялось.» + пояснение, что это P0-имитация, а не защита production-уровня; интерфейс не заявляет об управлении произвольными внешними агентами.
- Карточка «Предложенное действие»: textarea (пример $600), срок в секундах, кнопка «Предложить действие» → решение политики (бейдж эффекта, правило, сообщение) + capability (id, nonce, hash нагрузки, срок, состояние) или пояснение «не разрешено — capability не выдана, коннектор не вызывался».
- Карточка «Выполнение через поддельный коннектор» (только при выданной capability): селект канала (коннектор · учётная запись · окружение из реестра) + «Выполнить (имитация)» → квитанция: статус, причина отказа, сообщение поддельной транзакции, «коннектор вызван: да/нет».
- Списки «Квитанции» и «Capabilities» (свежими первыми) со статус-бейджами.
- `EffectBadge` вынесен из Policies.tsx в общий `src/pages/PolicyBadges.tsx` (без дублирования).

### TypeScript-тесты `tests/gateway.test.ts` (12)

- Точная метка имитации §4.11; лейблы всех 6 статусов; реестр зеркалит сид Rust; пример действия валиден и совпадает с демо (§4.11); `validateActionJson` (валидное, битый JSON, не-объект, пустые обязательные поля); `capabilityState` (ready/used/expired); `shortDigest`; константы TTL.

## Изменённые файлы

- `src-tauri/src/gateway.rs` — новый (нормализация, capability, поддельный коннектор, квитанции, реестр, 22 теста).
- `src-tauri/src/db.rs` — `init_gateway` в `init_db`; `wipe_all` + 4 таблицы gateway.
- `src-tauri/src/lib.rs` — `mod gateway;` + 4 команды.
- `src/data/gateway.ts` — новый (модель, метка имитации, реестр, пример действия, пред-валидация).
- `src/pages/GatewaySection.tsx` — новый (секция Gateway §4.11).
- `src/pages/PolicyBadges.tsx` — новый (общий `EffectBadge`).
- `src/pages/Policies.tsx` — вынесен `EffectBadge`, добавлена секция `<GatewaySection />`.
- `tests/gateway.test.ts` — новый (12 тестов).
- `docs/session-log/SESSION-12.md` — этот файл.

## Изменённые контракты

- Новый контракт: **gateway-sim/1** — таблицы `capabilities`, `gateway_receipts`, `gateway_connectors`, `gateway_meta`; семантика: нормализация → оценка → capability (action id, hash нагрузки, nonce, срок, однократность) → поддельный коннектор → квитанция со статусом имитации; реестр каналов сеется один раз за жизнь хранилища.
- Локальный протокол UI (не сетевой): `gateway_propose_cmd(action_json, ttl_seconds?)` → `GatewayProposal`; `gateway_execute_cmd(capability_id, connector_id, account_id, environment, action_json)` → `GatewayExecuteResult`; `list_gateway_receipts_cmd()` → `GatewayReceipt[]`; `list_gateway_capabilities_cmd()` → `CapabilityInfo[]`.
- Схема SQLite: + 4 таблицы (CREATE TABLE IF NOT EXISTS, миграций не требуется).

## Запущенные тесты

- `cargo test --lib`: **PASS** — 187 passed (включая 22 новых gateway + регресс wipe).
- `cargo clippy --all-targets`: **PASS** — без предупреждений (убран dead_code из `CapabilityRow`).
- `cargo fmt --check`: **PASS**.
- `cargo build`: **PASS** — приложение собирается целиком.
- `pnpm test`: **PASS** — 221 passed (включая 12 новых gateway).
- `pnpm typecheck`: **PASS**.
- `pnpm lint`: **PASS**.
- `pnpm format`: **PASS**.
- `pnpm build`: **PASS**.

## Проверка безопасности

- **Запрещённые возможности отсутствуют** (§4.11): нет сетевых вызовов, нет реальных внешних агентов; единственное «исполнение» — поддельный локальный коннектор, детерминированный и без побочных эффектов.
- **Capability — честная локальная имитация**: одноразовый nonce (uuid v4, UNIQUE), hash всей нагрузки, срок с fail-closed (непарсируемый срок = истёкший), однократное использование (`used_at` ставится только при реальной имитации, неудачные проверки capability не сжигают).
- **Порядок проверок при выполнении**: существование → повтор → срок → hash нагрузки → канал → повторная оценка политики. Любой отказ — квитанция `refused` с причиной и `connector_executed = false`; запрещённое действие никогда не достигает поддельного коннектора (проверено тестами).
- **Политика на обоих концах**: оценка и при предложении, и при выполнении (изменение правил после выдачи capability блокирует выполнение).
- **Квитанции без нагрузки**: квитанции и API не содержат исходного действия — только hash и метаданные; `action_json` живёт в локальной БД и не отдаётся наружу.
- **Валидация и лимиты**: JSON действия (16 КБ), обязательные поля, сумма ±1e12, число capabilities (500), число квитанций (2000).
- **SQL** — только параметризованные запросы; удаление — каскадно через `wipe_all` (SESSION-03).
- **Детерминизм**: hash нагрузки и результат поддельного коннектора детерминированы; nonce — единственный случайный элемент.
- **Интерфейс и документация не заявляют об управлении произвольными внешними агентами**: точная метка «Имитация: внешнее действие не выполнялось.», P0-имитация явно отделена от P1-изоляции.

## Влияние на производительность и токены

- Путь gateway: нормализация O(размер JSON) + оценка политики O(R) (R ≤ 500) + несколько параметризованных SQL — без модели: 0 токенов, 0 сети.
- Поддельный коннектор — O(размер действия), без I/O.

## Известные ограничения

> Устранены в review-pass ниже: канал привязан к capability, подпись и атомарность реализованы, confirm/redact имеют продолжение, реестр управляется из UI. Осталась только реальная изоляция учётных данных — сознательно P1 по §4.11.

- Capability не содержит канал (коннектор/учётка/окружение): по спецификации она несёт только action id, hash, nonce, срок, однократность; канал проверяется по локальному реестру на этапе выполнения. Связывание канала с capability — P1.
- `require_confirmation` и `redact` не имеют P0-продолжения: квитанция `held`/`redacted`, capability не выдаётся, поток подтверждения пользователем — P1.
- Реальная изоляция учётных данных, криптоподпись capability и атомарное выполнение — P1 (§4.11). P0-имитация никогда не выдается за защиту production-уровня.
- Реестр каналов статический (сид): управление коннекторами из UI — вне P0 (и вне «управления произвольными внешними агентами»).

## Примечания по реализации

- Квитанция предложения для разрешённого действия — `pending`; при выполнении обновляется до `simulated` (одна квитанция на действие), а каждая попытка выполнения дополнительно оставляет квитанцию `refused` с причиной — полный аудит-след.
- Канал в UI передаётся отдельными параметрами (`connector_id`, `account_id`, `environment`), а не извлекается из JSON действия — так отказ «неверный коннектор» можно продемонстрировать честно: нагрузка не меняется, меняется только канал.

## Коммит

- `5c2e2d81aaef1b9ea4edfc649651194830e628ec` `feat(soul): simulated gateway with capabilities and receipts [session-12]`

---

## Review-pass: устранены все ограничения и риски

Исправлены все пункты «Известных ограничений» первой версии. Capability и квитанции теперь подписаны локальным устройством (ed25519) — это закрывает и пробел со спецификацией: §4.11 требует «подписанную локальную квитанцию» (шаг демо), а в первой версии квитанции не были подписаны.

### Исправлено

1. **Capability привязывает канал (было: «не содержит канал, P1»).** Колонки `connector_id/account_id/environment` в `capabilities`; канал берётся из действия при выдаче и обязан быть в локальном реестре (иначе предложение отклоняется: «Channel … is not in the simulated connector registry»). Выполнение с другим каналом — отказ «capability bound to different channel»; удаление канала из реестра после выдачи — отказ «connector mismatch» (fail-closed).
2. **`require_confirmation` получил продолжение (было: «без P0-продолжения»).** Capability выдаётся в состоянии held (`confirmed_by_user = false`, квитанция `held`); новая команда `gateway_confirm_cmd` — локальное подтверждение пользователем (квитанция `held` → `pending`, переподпись capability, подпись покрывает состояние подтверждения); до подтверждения выполнение отказывается «confirmation required», отказ не сжигает capability. Повторное/лишнее подтверждение — ошибки.
3. **`redact` получил продолжение (было: «без P0-продолжения»).** Capability выдаётся с отредактированной копией действия (`redacted_json`): структурные поля сохраняются, чувствительные данные (получатель, домен, сумма, валюта, классы данных, scopes) скрыты. Поддельный коннектор «выполняет» именно отредактированный вариант (проверено: transaction_id соответствует hash отредактированного действия); квитанция — `simulated` с пометкой «payload redacted; no data exposed», эффект решения `redact` сохраняется.
4. **Криптоподпись capability и квитанций (было: «P1»).** Подпись ed25519 ключом локального устройства (существующий `crypto::ensure_device_keypair`, каталог keys) поверх канонического сообщения (все неизменяемые поля, включая канал и эффект решения). Подпись capability проверяется при выполнении **до** всех остальных проверок — любое вмешательство в строку (срок, канал, hash, nonce) → отказ «invalid capability signature» (fail-closed, проверено тестами). Квитанции подписываются при каждой записи (включая обновления held→pending и pending→simulated); при чтении подпись проверяется и в API отдаётся честный флаг `signature_valid` — подделанная квитанция не скрывается, а помечается (проверено тестом).
5. **Атомарное выполнение (было: «P1»).** Успешный путь (пометка `used_at` + обновление квитанции до `simulated`) и подтверждение (флаг + квитанция `held`→`pending`) — в одной транзакции `unchecked_transaction`. Отказ оставляет ровно одну квитанцию `refused`.
6. **Реестр каналов управляется из интерфейса (было: «статический сид»).** `list_gateway_connectors_cmd`, `gateway_add_connector_cmd`, `gateway_remove_connector_cmd` + секция «Реестр имитированных коннекторов» в UI (добавление/удаление каналов). Ограничения: пустые поля отклоняются, ≤64 символов на поле, ≤50 каналов (MAX_GATEWAY_CONNECTORS), добавление идемпотентно. Это по-прежнему локальная имитация — никаких реальных агентов.
7. **Повторная оценка политики при выполнении.** Жёсткий блок — только `Deny` (законное продолжение: для held/redact-capability политика на момент выполнения возвращает `require_confirmation`/`redact`, что уже согласовано на этапе выдачи; отказом это не является).

### Изменённые файлы (review-pass)

- `src-tauri/src/gateway.rs` — канал в capability, подпись (capability + квитанции), `confirm_capability`, redact-продолжение, транзакции, CRUD реестра, миграция `ensure_column` (ALTER TABLE ADD COLUMN для БД первой версии), 38 тестов.
- `src-tauri/src/lib.rs` — `gateway_device_keys(app)`; команды получают `app: AppHandle` и ключи; + `gateway_confirm_cmd`, `list_gateway_connectors_cmd`, `gateway_add_connector_cmd`, `gateway_remove_connector_cmd`.
- `src/data/gateway.ts` — `GatewayCapability` (канал, `decision_effect`, `confirmed_by_user`, `redacted`, подпись, `signature_valid`), `GatewayReceipt` (подпись, `signature_valid`), `GatewayChannel` (snake_case), `validateChannelInput`, `capabilityState` → `held`, лимиты реестра.
- `src/pages/GatewaySection.tsx` — подтверждение held-capability кнопкой «Подтвердить (пользователь)», живой реестр (список/добавить/удалить), селект канала из реестра, бейджи подписи «✓ подписано / ✕ подпись недействительна».
- `tests/gateway.test.ts` — +5 тестов (проверка канала, held-состояние, лимиты реестра).
- `docs/session-log/SESSION-12.md` — этот раздел.

### Тесты (review-pass)

- `cargo test --lib`: **PASS** — 200 passed (38 gateway: привязка канала ×3 + удалённый из реестра канал, подделка подписи capability (срок/канал) → «invalid capability signature», подделка квитанции → `signature_valid=false`, confirm-поток ×5, redact-продолжение, CRUD реестра ×2, предложение для канала вне реестра, регресс первого прохода).
- `cargo clippy --all-targets`: **PASS** — без предупреждений.
- `cargo fmt --check`: **PASS**.
- `cargo build`: **PASS**.
- `pnpm test`: **PASS** — 226 passed (17 gateway).
- `pnpm typecheck` / `pnpm lint` / `pnpm format` / `pnpm build`: **PASS**.

### Остаточные ограничения (сознательно P1)

- Реальная изоляция учётных данных (настоящие внешние коннекторы, криденшелы вне процесса) — явно P1 по §4.11 («Настоящая изоляция учётных данных … относятся к P1»); P0-имитация не выдаётся за защиту production-уровня, метка §4.11 сохранена. Локальная атомарность и подпись закрывают целостность в границах P0, но не заменяют изоляцию P1.

## Коммит (review-pass)

- `bd0b155` `fix(soul): review-pass session-12 — channel-bound signed capabilities, confirmation flow, redaction continuation, atomic execution, editable connector registry [session-12]`

---

## Ultra-review (SESSION-12): все ошибки сессии

Повторная систематическая проверка всей сессии (первая версия + review-pass) по коду и каждому тесту. Найденное и исправленное:

### Найдено и исправлено

1. **Пробел в целостности сохранённой нагрузки (главное).** Подпись capability покрывает hash нагрузки, но не само сохранённое `action_json`/`redacted_json`: локальное вмешательство в эти колонки подписью не ловилось (подпись оставалась валидной), а подделанное действие попадало в повторную оценку политики и в поддельный коннектор. Исправление: при выполнении сохранённое действие повторно хешируется и сверяется с подписанным `payload_hash`, `redacted_json` сверяется с каноническим отредактированным вариантом; расхождение — отказ «stored action tampered» (fail-closed, как остальные подделки). Тесты: `tampered_stored_payload_is_refused`, `tampered_redacted_variant_is_refused`.
2. **Рассинхрон обязательных полей.** `environment` отсутствовал в проверке обязательных полей `normalize_action` (Rust) и `validateActionJson` (TS), хотя `validateChannelInput` его требует; пустое окружение доходило до общей ошибки канала реестра. Исправлено в обоих зеркалах; кейс добавлен в `normalize_rejects_missing_required_fields`.
3. **Нетранзакционность `propose_action`.** Capability и квитанция вставлялись раздельно — при сбое вставки квитанции (лимит 2000) оставалась осиротевшая capability без квитанции. Теперь обе записи в одной транзакции, сбой откатывает capability.
4. **Числа в журнале.** Gateway-тестов было 25 в первой версии и 38 после review-pass, а не 22 и 35 (итоги 187/200 были верны, недосчёт — только в описании). Исправлено здесь.

### Проверка после исправлений

- `cargo test --lib`: **PASS** — 202 passed (40 gateway: +2 на целостность сохранённой нагрузки, +кейс environment).
- `cargo clippy --all-targets`: **PASS** — без предупреждений. `cargo fmt --check`: **PASS**.
- `pnpm test`: **PASS** — 226 passed (17 gateway). `pnpm typecheck` / `pnpm lint` / `pnpm format` / `pnpm build`: **PASS**.

## Коммит (ultra-review)

- `8be7fb7` `fix(soul): session-12 ultra-review — stored-payload integrity re-hash, environment required in normalize_action, atomic propose [session-12]`

---

## Project-wide ultra-review: все компоненты (Rust + TS + расширение + конфиги)

Систематическая проверка всего проекта четырьмя ревью-агентами (Rust-бэкенд, TS-фронт, браузерное расширение, конфиги/спека) с последующей ручной верификацией каждого пункта по коду. Найдено и исправлено:

### Найдено и исправлено

1. **CRITICAL: браузерный relay принимал сообщения от любого отправителя.** `background.ts` не проверял `sender.id` в `chrome.runtime.onMessage`: любое другое установленное расширение могло вызвать `chrome.runtime.sendMessage(наш_id, …)` и получить контекст пользователя (проверки `extensionId`/`origin` в `validateOutgoingRequest` работали с самозаявленными полями сообщения). Исправление: `isTrustedSender` — сообщения принимаются только с `sender.id === EXTENSION_ID` (поле, которое Chrome не даёт подделать); прочим — `soul.error` `invalid_sender` без обращения к native host. Тесты: `tests/browser-sender.test.ts` (мок `chrome` + динамический импорт модуля).
2. **HIGH: неатомарный импорт пакета.** `import_package_file` вызывал `db::wipe_all` вне транзакции: валидно подписанный пакет с невставляемыми данными (дубль id) уничтожал существующий SOUL без восстановления (нарушение §4.3/§4.12). Исправление: `wipe_all_tx(&tx)` внутри той же транзакции, что и вставки (`VACUUM`/checkpoint остались в `wipe_all` отдельно — внутри транзакции они запрещены). Тест: `failed_import_with_invalid_payload_preserves_existing_soul` (сборка подписанного пакета из произвольного payload).
3. **MEDIUM: вопросы типа `multiple` рендерились как radio.** В `Calibration.tsx` выбор нескольких тем (bound_1/bound_2) был невозможен, ответы уходили строкой. Исправление: checkboxes с toggle-логикой в массив; `compile.ts` распознаёт «Nothing is off-limits» в массиве (dispute-правило). Тесты: массивы в `tests/compile.test.ts`.
4. **MEDIUM: ошибка сохранения калибровки проглатывалась.** `handleSaveCalibration` ловил ошибку и не перевыбрасывал — `handleNext` продвигался дальше, ответы фактически не сохранялись. Теперь ошибка перевыбрасывается, `handleNext` не продвигает шаг при сбое (ошибка показана родителем).
5. **LOW: unbounded чтение строк в MCP.** `serve_io` читал `lines()` без лимита. Введён `MCP_MAX_LINE_BYTES = 4 МБ` (дефолт MCP SDK), `read_until` + дочитывание остатка строки, превышение — parse-ошибка, цикл продолжается. Тест: `serve_io_rejects_oversized_line_and_continues`.
6. **LOW: read-only-фолбэк молча открывал БД на запись.** В `bridge.rs` и `mcp.rs` при отсутствии `-wal`/`-shm` read-only-открытие падало на read-write. Исправление: после фолбэка `PRAGMA query_only=ON` — ни одна запись в таблицы невозможна. Тест: `read_only_connection_rejects_writes_even_after_fallback`.
7. **HIGH: CSP отключён + мёртвый shell-плагин.** `tauri.conf.json` `csp: null`; `shell:allow-open` в capabilities без единого вызова + `tauri_plugin_shell::init()`. Исправление: строгий CSP (`default-src 'self'`, без remote/`data:` для скриптов, `frame-ancestors 'none'`, `object-src 'none'`; `'unsafe-inline'` только для script/style из-за vite dev и inline-стилей React), удалены зависимость `tauri-plugin-shell`, инициализация и привилегия.
8. **LOW: несоответствие типов версий протокола.** `protocol.ts` объявлял `policyVersion`/`stateVersion` как `number`, host шлёт строки (8 hex, `context.rs`). Типы исправлены, `isContextResponse` теперь проверяет строковый тип. Тесты: `tests/browser-protocol.test.ts`.

### Проверка после исправлений

- `cargo test --lib`: **PASS** — 205 passed (+3: атомарный импорт, лимит строки MCP, query_only).
- `cargo clippy --all-targets`: **PASS** — без предупреждений. `cargo fmt --check`: **PASS**.
- `pnpm test`: **PASS** — 235 passed (+9: sender-валидация ×4, массивы в compile ×3, isContextResponse ×2 из 3 новых).
- `pnpm typecheck` / `pnpm lint` / `pnpm format` / `pnpm build:companion`: **PASS**.

## Коммит (project-wide ultra-review)

- `e56dc2d` `fix(soul): project-wide ultra-review — trusted-sender gate in browser relay, atomic import wipe, calibration multi-select and save errors, MCP line cap + query_only, CSP, drop dead shell plugin [session-12]`
