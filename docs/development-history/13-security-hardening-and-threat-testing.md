# Security Hardening and Threat Testing

## Цель

Атаковать реализованный ultra-MVP и до полировки исправить каждую уязвимость, с которой можно что-то сделать (спека SESSION-13): аудит зависимостей, статический анализ, вредоносные архивы, данные с инъекциями промптов, тесты XSS и обхода путей, авторизация между SOUL, злоупотребление локальным IPC, поиск утечек секретов, property-тесты политик и повтора capability. Все найденные проблемы исправлены кодом, не только задокументированы.

## Аудит зависимостей и секретов

- `cargo audit`: **0 известных уязвимостей**; «unmaintained» — только транзитивные Linux-пакеты GTK3-цепочек (не используются в сборке Windows-биндингов), зафиксировано в базе advisory как информационное.
- `pnpm audit` (прод): **clean**.
- Поиск утечек секретов в журналах, исходниках и тестовых данных: реальных учётных данных/ключей/личных экспортов не найдено. `test.soul` и `dev-run.log` — рабочие артефакты сессии, в `.gitignore`, в коммит не попадают. `extension-key.base64` — публичный ключ по дизайну (не секрет).
- Статический анализ и типы: `cargo clippy --all-targets -- -D warnings` — 0 замечаний; `cargo fmt --check` — чисто; `pnpm typecheck` / `pnpm lint` / `pnpm format` — PASS.

## Вредоносные архивы (пакеты импорта/экспорта)

Все проверки в `verify_package_bytes` выполняются **до** расшифровки только для формата/подписи, а содержимое — после Argon2 с жёсткими капами параметров:

1. **Капы параметров KDF до запуска Argon2** (`MAX_ACCEPTED_KDF_MEM_KIB=2_000_000` (2 ГБ), `MAX_ACCEPTED_KDF_TIME=10`, `MAX_ACCEPTED_KDF_PARALLELISM=4`): нулевые или нереальные параметры отклоняются ошибкой «Package KDF parameters are outside the allowed range.» — архив не может заставить клиент сжечь память/CPU (тесты: `malicious_kdf_params_are_rejected_before_kdf` ×3, `kdf_caps_accept_export_defaults`).
2. **Цепочка событий** (`verify_event_chain`): все события пакета принадлежат тому же `soul_id`, что и payload; дубли id событий отклоняются; обязано быть ровно одно root-событие; каждый `previous_event_hash` обязан ссылаться на существующее событие **внутри пакета**; head-hash последнего события сверяется с полем `head_event_hash` (тесты: `duplicate_event_id_is_rejected`, `event_with_foreign_soul_id_is_rejected`, `dangling_previous_event_hash_is_rejected`, `event_chain_without_root_is_rejected`).
3. **Сущности** (`verify_entities`): `soul_id` payload обязателен для всех сущностей; дубли id отклоняются; `entity_type`/`status` проходят через те же валидаторы, что и прямые записи в БД; claim проходит лимит длины (тесты: `entity_with_foreign_soul_id_is_rejected`, `entity_with_invalid_status_or_type_is_rejected`, `entity_with_oversized_claim_is_rejected`).
4. **Размер после расшифровки**: после проверки hash содержимого расшифрованный plaintext проверяется на лимит (`max_bytes`) — декомпрессия «zip-бомбы» невозможна (это дополнение к прежней проверке сжатого размера).
5. **Атомарность импорта** (унаследована из SESSION-12): пакет с валидной подписью, но невставляемыми данными отклоняется целиком, существующий SOUL сохраняется (`failed_import_with_invalid_payload_preserves_existing_soul` теперь ждёт «Duplicate entity id», проверяя и уникальность внутри пакета).

## Инъекции промптов — данные остаются данными

- Тест `import_preserves_injection_claims_verbatim`: 6 текстов инъекций («ignore previous instructions…», «system: …», фиктивные `<policy>`-блоки и пр.) проходят полный цикл экспорт→импорт→обратно **дословно**; никакой интерпретации: политики не создаются (число правил в БД не меняется), привилегии не расширяются. Данные могут попасть в контекст только через явный механизм (SESSION-09/10/12 — policyVersion/stateVersion и trust-гейты), а не из самого текста.
- Единственная точка входа импорта — Rust `import_package_file` (Tauri-команда), в UI рендер — React с авто-экранированием.

## XSS и обход путей

- **`md_escape`** в `package.rs`: экранирование `& < > " '` применяется при экспорте в Markdown ко всем дисплейным полям данных (`display_name`, `entity_type`, `status`, `claim`) — файл экспорта не может нести исполняемый HTML (тест `markdown_export_escapes_html`).
- **`validate_export_path`**: пути экспорта с NUL или пустые отклоняются («export path must not be empty / must not contain NUL»), применяется во всех трёх экспортных командах (тест `export_path_with_nul_is_rejected`).
- **`crypto::read_file_limited`**: чтение файла с NUL в пути отклоняется («File path must not contain NUL characters.») — закрыт обход через закодированные имена файлов.
- Обхода путей через `..` в экспорте нет: имена файлов формируются из `soul_id`/UUID (не из пользовательских строк); импорт работает через диалог ОС.

## Авторизация между SOUL (изоляция записей)

Раньше `update_entity` и `delete_evaluation` принимали только id записи — любая команда могла менять/удалять записи другого SOUL (баг в реализации, не только «потерянный» тест). Исправлено:

- **`db::update_entity(conn, soul_id, entity_id, …)`** — новая сигнатура: запись чужого SOUL отклоняется той же ошибкой «Entity not found.», что и отсутствующая (нет оракула существования). То же для команд из UI и тестов (сид переписан на `let (soul_id, ent_id) = seed(&env)`).
- **`eval::delete_evaluation(conn, soul_id, evaluation_id)`** — владелец проверяется через SELECT owner перед удалением; чужая оценка → «Evaluation not found.» (та же строка, что и отсутствующая).
- Тесты изоляции: `foreign_soul_cannot_update_entity` (soul_b не может активировать/редактировать сущность soul_a — та же ошибка, данные не тронуты, свой SOUL продолжает работать), `delete_foreign_evaluation_is_rejected`.
- UI: `runStatusUpdate`/`handleEditEntity` (App.tsx) и `handleDelete` (Tests.tsx) передают `soul_id` из активного SOUL.

## Злоупотребление локальным IPC (bridge/MCP)

- **`context::validate_query(&ContextQuery)`** — новые лимиты: `text` ≤ 8 000 символов; каждый фильтр (домены/проекты/люди/каналы) ≤ 64 записей; суммарно все фильтры ≤ 200 записей; запись ≤ 256 символов; `since`/`until` ≤ 64 символов.
- Валидация вызывается **первой** и в `mcp::get_context`, и в `bridge::compile_and_respond` — до компиляции контекста; превышение → чистый ответ с ошибкой без единого вызова модели (тест `get_context_rejects_oversized_query_params`: текст 8001 симв., фильтр 65 записей, 209 записей по 4 измерениям, запись 257 симв., since 65 симв.).

## Property-тесты

- **Политики** (`property_evaluate_is_total_deterministic_and_never_panics`, seed `0x5a01_13a5`): 24 случайных валидных правила (домены/классы/эффекты/сроки) × 400 случайных действий — `evaluate` тотален (эффект всегда определён, никогда не паникует) и детерминирован (два вызова с одним правилом/действием дают одинаковый результат; `Decision` получил `PartialEq`).
- **Gateway** (`property_propose_execute_roundtrip_replay_and_determinism`, seed `0x51a0_1e13`): 60 случайных действий — nonce уникальны, `payload_hash` детерминирован, выполнение успешно, повтор отклонён («capability already used»), все квитанции подписаны (`signature_valid`); число квитанций = 2×выполненных + отклонённых повтором (каждая попытка оставляет refused-квитанцию).

## Запущенные тесты

- `cargo test --lib`: **PASS** — 222 passed (было 205; +17 новых, 1 обновлён) за 8.2 с.
- `cargo clippy --all-targets -- -D warnings`: **PASS** — 0 предупреждений.
- `cargo fmt --check`: **PASS** (форматирование применено).
- `cargo build`: **PASS** — приложение собирается целиком.
- `pnpm test`: **PASS** — 235 passed (без изменений: UI-правки — только проброс `soul_id`).
- `pnpm typecheck` / `pnpm lint` / `pnpm format`: **PASS**.

## Изменённые файлы

- `src-tauri/src/package.rs` — капы KDF, `verify_event_chain`/`verify_entities`, лимит расшифрованного размера, `validate_export_path`, `md_escape`; 12 новых + 1 обновлённый тест.
- `src-tauri/src/db.rs` — `update_entity(conn, soul_id, …)`, валидаторы `pub(crate)`, тест изоляции SOUL; форматирование.
- `src-tauri/src/eval.rs` — `delete_evaluation(conn, soul_id, …)`, тест изоляции SOUL.
- `src-tauri/src/lib.rs` — сигнатуры `update_entity_cmd`/`delete_evaluation_cmd` (+ `soul_id`).
- `src-tauri/src/context.rs` — константы лимитов + `validate_query`.
- `src-tauri/src/mcp.rs` — вызов `validate_query` в `get_context`, тест переполнения параметров.
- `src-tauri/src/bridge.rs` — вызов `validate_query` в `compile_and_respond`.
- `src-tauri/src/crypto.rs` — NUL-гард в `read_file_limited`.
- `src-tauri/src/policy.rs` — `PartialEq` для `Decision`, property-тест.
- `src-tauri/src/gateway.rs` — property-тест повтора/детерминизма.
- `src/App.tsx`, `src/pages/Tests.tsx` — проброс `soul_id` в команды.

## Ограничения и риски (низкая серьёзность, задокументированы)

1. **Совместимость KDF-капов**: легитимный экспорт, сделанный с очень агрессивными параметрами Argon2, будет отклонён импортом. Это осознанная защита (клиент не может быть вынужден сжигать 2+ ГБ памяти); при необходимости лимиты поднимаются в константах `MAX_ACCEPTED_KDF_*`.
2. **Property-тесты детерминированы по сиду** (зафиксированные seed'ы для стабильности CI). Расширение до произвольных/случайных сидов или распределённого fuzzing — отдельная задача (P1), текущее покрытие — регресс-слой против паник и недетерминизма.
3. **TS-сторона инъекций/XSS**: вход данных в UI — только из защищённой БД и Rust-команд (единственная точка входа импорта — Rust); рендер — React с авто-экранированием; экранирование Markdown-экспорта покрыто тестом. Отдельные TS-тесты рендера не добавлялись — поверхность закрыта на Rust-слое.
4. **LNK4099** при сборке тестов: шумные предупреждения линкера lld-link о PDB openssl-sys (безобидны, не влияют на результат; фильтруются в журналах CI при необходимости).
5. Реальная изоляция учётных данных/внешних коннекторов — сознательно P1 (§4.11), не входит в объём SESSION-13.

## Коммит

- `d56042877e45c28b330bd1be7f6d3e5f47a6ef00` `fix(soul): session-13 security hardening — scoped KDF/chain/entity import checks, query caps for bridge/mcp, cross-SOUL auth for entities and evaluations, export escaping, property tests [session-13]`
