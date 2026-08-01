# SESSION-08 — Локальный MCP-сервер и интеграции с AI-клиентами

## Цель

Дать SOUL выход наружу безопасным и контролируемым способом: локальный фоновый runtime с узким API `soul.get_context` и disclosure-квитанцией, собственный локальный MCP-сервер (stdio, JSON-RPC 2.0) для Claude Code / Codex / Cursor, обнаружение клиентов, подключение/отключение с backup/verify/rollback. Без BYOK, без собственного чата, без угадывания клиента.

## Реализовано

### Rust: компилятор контекста `src-tauri/src/context.rs` (новый)

Полный порт TS-компилятора из SESSION-07, результат байт-в-байт идентичен TS-реализации:

- Константы: `CONTEXT_POLICY_VERSION = "soul-context-policy/1"`, `CONTEXT_STANDARD_TOKENS = 900`, `CONTEXT_HARD_MAX_TOKENS = 3000`.
- `estimate_tokens` — та же консервативная оценка (CJK ≈ 1, остальные ≈ 1/3, посимвольное float-сложение как в TS, `ceil`), `format_tokens` — разделители тысяч как `toLocaleString('en-US')`.
- `hash_string` — 32-битный FNV-1a поверх UTF-16 code units (wrapping_mul = imul); векторa: `"" → 811c9dc5`, `"a" → e40c292c`, `"foobar" → bf9cf968`.
- `sensitivity_of` fallback `"internal"`, `domains_of` = `scope.domains` с fallback на `domainForQuestion`, `relevance_of` (claim ×2, evidence ×1), `dedupe_superseded`, `detect_conflicts` (serde_json canonical key, пары по id), жёсткая упаковка бюджета с учётом заголовка и `CONFLICTS:`/`SUPERSEDED:` и страховочным сбросом при замене «X».
- **Golden-тест (кросс-языковой)**: фикстура ent_a/ent_b/ent_c, полный литерал сериализованного пакета, `state: 5b38f537`, `tokens: 110 of 900`, `token_estimate == 110` — зафиксированы одинаковыми константами в Rust и TS (`tests/context.test.ts`), оба проходят.
- 15 тестов: golden, детерминизм при обратном порядке входа, смена state-версии, статусы, чувствительность, scope+домены, временное окно, текстовый запрос, бюджет (soft/hard, cap 99 999 → 3000, 0 → 1), пустой пак, расход заголовка/конфликтов в бюджет.

### Rust: MCP-сервер `src-tauri/src/mcp.rs` (новый)

Без внешнего MCP-крейта — свой JSON-RPC 2.0 поверх stdio с разделителями строк (единственная новая зависимость проекта — `toml = "0.8"`):

- `MCP_PROTOCOL_VERSION = "2025-06-18"`, сервер `soul-mcp`, тул `soul.get_context`, промпт `soul.task_start`, коды ошибок -32700/-32600/-32601/-32602.
- Методы: `initialize` (capabilities tools+prompts, serverInfo), `ping`, `tools/list`, `tools/call`, `prompts/list`, `prompts/get`, notifications игнорируются (нет id → нет ответа).
- `soul.get_context`: входные аргументы `text/domains/projects/people/channels/sensitivity (enum 5)/statuses/since/until/maxTokens (1–3000, default 900)`; ошибка -32602 на невалидный `maxTokens`. Читает **только первую душу** (`db::list_souls` + `db::list_entities`), компилирует пак и возвращает два text-content: сериализованный пак + metadata JSON (policyVersion/stateVersion/entityCount/maxTokens/tokenEstimate/conflicts/supersededIds).
- Disclosure-квитанция пишется **до** ответа и не содержит текста задачи, query, id сущностей, claim или секретов — только kind, disclosed_at, client, entity_count, token_estimate, policy_version, state_version, max_tokens.
- `resolve_app_dir`: env `SOUL_APP_DIR` → `%APPDATA%/ai.soul.runtime` (Windows) → `~/.local/share/ai.soul.runtime`; клиент из env `SOUL_MCP_CLIENT` (default `"unknown"`), без угадывания.
- `open_app_db`: READ_ONLY, fallback READ_WRITE (WAL без `-shm`).
- `TASK_START_INSTRUCTIONS` (промпт): вызов тула добровольный, конфликты и замещённые ответы — справочные, всё локально.
- 12 тестов: initialize, ping, notifications без ответа, схемы tools/prompts, get_context + квитанция (в квитанции нет id/claim/секретов), фильтры/бюджет/isError, неизвестный тул/метод, ошибки парсинга, serve_io roundtrip, пустая БД, нет БД.

### Rust: интеграции `src-tauri/src/integrations.rs` (новый)

- Клиенты: `claude-code` → `~/.claude.json`, `codex` → `~/.codex/config.toml`, `cursor` → `~/.cursor/mcp.json`; бинарь сервера — `soul-mcp(.exe)` рядом с текущим exe; `home_dir` = `USERPROFILE` → `HOME`.
- Вставка: JSON — pretty `mcpServers.soul {command, args: []}`; TOML — секция `[mcp_servers.soul]` с re-parse-валидацией. Удаление точечное: JSON — только ключ `soul`; TOML — только секция `[mcp_servers.soul]`; чужие/пользовательские записи не трогаются.
- `atomic_write` = temp+rename, фолбэк remove+rename для Windows.
- `IntegrationState` в `app_dir/integrations/<client>.json`: `{client, config_path, backup_path, original_hash, written_hash, connected_at}`. Отключение: `sha256(текущий) == written_hash` → restore backup (или удалить файл, если до подключения его не было: `original_hash == sha256("")`); иначе — хирургическое удаление с сохранением пользовательских изменений. Каждая операция читает файл обратно и сверяет.
- Fail-closed: непарсируемая конфигурация → поле `error`, подключение запрещено; чужой `soul`-элемент с другим command → ошибка, не перезапись.
- **Тесты изолированы**: реализация разбита на `*_for(home, ...)` варианты, публичные обёртки читают `home_dir`; 10 тестов (JSON и TOML connect/disconnect/rollback, idempotence, отказ на чужой записи, fail-closed invalid JSON, создание отсутствующей конфигурации, восстановление backup, хирургическое удаление, сохранение `[model]` и чужих mcp_servers).

### Rust: командный слой `src-tauri/src/lib.rs`

- Новые команды: `detect_clients_cmd`, `connect_client_cmd`, `disconnect_client_cmd`, `rollback_client_cmd` (все с `tauri::AppHandle`, `app_data_dir`, парсингом `ClientId`) — зарегистрированы в invoke_handler.
- `src-tauri/src/package.rs`: `DisclosureReceipt` + `write_disclosure_receipt` → `receipts/disclosure-{Uuid}.json`; `ReceiptSummary` стал union-строкой `{file, kind ("deletion"|"disclosure"), at, entity_count, event_count?, keys_deleted?, client?, token_estimate?, policy_version?, state_version?}`; `list_local_receipts` парсит disclosure первым, затем deletion, повреждённые файлы пропускаются.
- `src-tauri/src/bin/soul_mcp.rs` (новый): `resolve_app_dir` → `serve_stdio`; ошибки в stderr, exit 1.

### TypeScript: модель и UI

- **`src/data/integrations.ts` (новый)**: чистые функции для UI — `isClientId`, `clientLabel`, `clientStateLabel` (Connected/Detected/Not found/Error), `clientAction` (connect/disconnect/none), `clientStatusNote`, `clientActionLabel`.
- **`src/data/control.ts`**: CTA активного состояния разблокирован — `connect-ai-client` теперь `disabled: false`, note «Your SOUL is active and local. Connect a supported AI client (Claude Code, Codex, Cursor) in Settings.»; `tests/control.test.ts` обновлён.
- **`src/pages/Settings.tsx`**: новая секция «AI clients» (обнаружение через `detect_clients_cmd`, кнопки Connect/Disconnect/Rollback с блокировкой по состоянию, статус-нота под карточкой, ошибки поверх секции); интерфейс `ReceiptSummary` расширен до union-полей, секция «Local receipts» рендерит disclosure-строки (клиент, ~токены, state, policy) и deletion-строки.
- **`src/App.tsx` + `src/pages/Home.tsx`**: Stat «Connected AI clients» больше не хардкод — счётчик из `detect_clients_cmd` (загрузка при старте и при входе во вкладку Settings).

## Изменённые файлы

- `src-tauri/Cargo.toml`: `toml = "0.8"`, `[[bin]] soul-mcp → src/bin/soul_mcp.rs`.
- `src-tauri/src/context.rs` — новый (порт компилятора).
- `src-tauri/src/mcp.rs` — новый (JSON-RPC 2.0 stdio).
- `src-tauri/src/integrations.rs` — новый (клиенты/backup/rollback).
- `src-tauri/src/bin/soul_mcp.rs` — новый (entry).
- `src-tauri/src/lib.rs`, `src-tauri/src/package.rs` — команды и квитанции.
- `src/data/integrations.ts` — новый (модель статусов).
- `src/data/control.ts`, `tests/control.test.ts` — CTA активного состояния.
- `src/pages/Settings.tsx`, `src/pages/Home.tsx`, `src/App.tsx` — секция AI clients, disclosure-квитанции, счётчик подключений.
- `tests/context.test.ts` — кросс-языковой golden-тест.
- `tests/integrations.test.ts` — новый (13 тестов модели).

## Как работает

Пользователь открывает Settings → AI clients: приложение зовёт `detect_clients_cmd` (пути конфигов, существует ли бинарь сервера, подключён ли уже, есть ли backup). Connect: конфиг клиента бэкапится (даже отсутствующий — пустой backup), вставляется запись `mcpServers.soul`, файл читается обратно и сверяется, при сбое — rollback. Отключается так же атомарно: если файл не меняли после подключения — restore backup/удаление; если меняли — только точечное удаление записи soul. При старте внешнего клиента его MCP-рантайм запускает `soul-mcp.exe`; на `soul.get_context` сервер читает первую душу, компилирует пак (детерминированно), пишет disclosure-квитанцию и отвечает. Квитанции видны в Settings → Local receipts. Пользователь видит «N of 3 supported clients connected» на Home.

## Инцидент: тесты записали в реальный `~/.claude.json` (исправлено)

Ранние версии тестов интеграций использовали env `home_dir()` — `cargo test` добавил запись `mcpServers.soul` с temp-путём бинаря в реальный `C:\Users\eugene\.claude.json`. Обнаружено ручной проверкой после прогона; найден backup `.claude.json.soul-backup-b2540fb9-c347-4c85-a77c-845a56e2dfb6-20260731211028` (в нём записи `soul` не было); файл восстановлен, backup удалён, `"soul"` в конфиге = false.

**Меры**: реализация разбита на тестируемые `*_for(home, ...)` варианты; тесты работают только с temp-home; публичные обёртки читают `home_dir` один раз. Впредь никакой тест не имеет доступа к реальной конфигурации пользователя.

## Review pass (после сдачи)

Повторное ревью всех изменений сессии выявило и исправило:

- **Мусорная БД в рабочей папке** (найдено ультра-ревью): заготовка `AppState` в `lib.rs` выполняла `init_db(".")` до `.setup()` — при каждом запуске в CWD создавалась `soul.db` (49 КБ), которую потом перезаписывал реальный коннект из `app_data_dir`. Исправлено: заглушка — in-memory коннект без обращения к диску; реальная БД создаётся только в `setup()`/`init_app`. Мусорный файл удалён, повторно не создаётся.
- **Rollback при сбое записи после connect**: если `write_state` падал после успешного изменения конфига, состояние (backup, хэш) не откатывалось — теперь `rollback_connect()` + удаление backup + ошибка «Cannot persist integration state; config rolled back».
- **Rollback при отсутствовавшем ранее конфиге**: если файла не было до подключения (`original_hash == sha256("")`), rollback писал пустой файл — теперь файл удаляется; проверка ветки — `!config_path.exists()`.
- **Счётчик на Home не обновлялся**: `loadConnectedClients` в App.tsx вызывался только при входе во вкладку Settings — теперь при любой смене таба.
- **E2E-тест реального бинаря** `real_binary_serves_context_over_stdio` (mcp.rs): спавнит настоящий `soul-mcp.exe` (skip, если не собран), initialize → `tools/call soul.get_context` по stdio, проверяет пак («entities: 2», «Prefers concise answers», «Never share medical data») и disclosure-квитанцию в `receipts/`, завершение по EOF. Первый прогон показал ошибочное ожидание теста (`split("entity")`), не баг рантайма — ожидание заменено на contains-проверки.
- Мёртвые команды не обнаружены: `init_app` используется фронтендом; `activate_soul_cmd` зарегистрирован, но фронтенд его не вызывает (оставлен как API).

## Тесты и проверки

- `cargo test`: PASS — 104/104 (context 15, mcp 13 (+E2E), integrations 11 (+rollback без файла), плюс прежние).
- `cargo clippy --all-targets`: без предупреждений.
- `pnpm test`: PASS — 116/116 (+13 integrations, +2 кросс-языковых golden/дедуп, control обновлён).
- `pnpm typecheck`, `pnpm lint`, `pnpm build`: PASS.
- Кросс-языковой golden: один и тот же фикстурный пак в Rust и TS даёт байт-в-байт одинаковый serialized, `state: 5b38f537`, `tokens: 110 of 900`, `token_estimate == 110`.

## Проверка безопасности

- **Угрозы**: раскрытие всего SOUL наружу, утечка секретов/claim/id в квитанции, повреждение конфигурации клиента, потеря пользовательских изменений конфига, перезапись чужого `soul`-элемента, угадывание клиента/путей, небезопасная запись файлов.
- **Меры**: MCP-API узкий (один тул, жёсткий бюджет токенов); disclosure-квитанция содержит только метаданные (тест проверяет отсутствие id/claim/секретов); backup + verify + rollback на каждой операции; отключение различает «файл не менялся» (restore) и «пользователь менял» (хирургия); чужой soul-элемент → отказ, а не перезапись; fail-closed на битом JSON/TOML; client из явного env, без угадывания; атомарная запись (temp+rename); READ_ONLY-открытие БД.

## Производительность

- `soul.get_context`: два SQL-запроса + один проход компилятора — миллисекунды; компилятор уже замерен (p95 < 75 мс на 300 сущностях).
- MCP-общение — NDJSON строки, без сериализации всего состояния.

## Известные ограничения

- `soul.get_context` работает с первой душой (одна душа — текущий сценарий).
- Тул не принимает `scope` как структуру — только домены/проекты/люди/каналы списками (как в TS-запросе).
- Rollback в UI доступен только если есть backup-файл от последнего подключения.
- MCP-сервер не реализует ресурсы/логирование — только tools и prompts (достаточно для задачи).

## Последующие сессии

- Реальное подключение Claude Code/Codex/Cursor и проверка рабочего процесса «задача → get_context → disclosure».
- UI-кейсы «подключён → CTA на Home меняется», уведомления о новых квитанциях.

## Коммиты

- `ed692de` — `feat(soul): local MCP server, context compiler port and AI client integrations [session-08]`
- `f3c5c1f` — `docs(soul): record session-08 commit hash in session log [session-08]` (повторён в `90150a7`)
- `6f240d9` — `fix(soul): review-pass - no stray db in cwd, rollback without prior config, write_state rollback, binary e2e test, connected clients refresh [session-08]`
