# SESSION-09 — Browser Companion для веб-чатов

## Цель

Сделать SOUL автоматически доступным в обычных ChatGPT Web, Gemini Web и Claude Web без ручного копирования контекста: Chromium-расширение MV3 общается с локальным runtime через зарегистрированный Native Messaging host, при одном нажатии пользователем Send приостанавливает отправку, запрашивает разрешённый контекст локально, вставляет структурированный SOUL context в то же сообщение и продолжает ту же отправку без второго клика. Всё происходит строго внутри исходного веб-чата; расширение не читает другие вкладки, cookies, пароли или историю браузера; контекст не сохраняется; при изменении разметки сайта адаптер отключается по fail-closed; отключение host-а возвращает чат к обычному поведению.

## Реализовано

### Rust: Native Messaging host `src-tauri/src/bridge.rs` (новый)

- Протокол `soul-bridge/1`; запросы `soul.ping → soul.pong`, `soul.get_context → soul.context {pack, entityCount, tokenEstimate, policyVersion, stateVersion, maxTokens}`; ошибки — `soul.error` с кодами `invalid_protocol / invalid_extension_id / invalid_nonce / invalid_origin / task_too_long / request_too_large / replay_detected / invalid_request / unsupported_request / runtime_error`.
- Кадры: `u32 LE длина + payload`, лимит 1 МБ (`BRIDGE_MAX_FRAME_BYTES` — жёсткий предел Chrome native messaging); `read_frame`/`write_frame` с ошибкой на oversized-кадр.
- Валидация каждого запроса: версия протокола, extension ID (`BRIDGE_EXTENSION_ID`, переопределяется через env `SOUL_BRIDGE_ALLOWED_EXTENSION_IDS` для тестов), формат nonce (16–64 символа `[A-Za-z0-9_-]`), replay-защита через `BridgeSession.seen_nonces`, для `soul.get_context` — происхождение из строгого списка `SUPPORTED_ORIGINS` (`https://chatgpt.com`, `https://gemini.google.com`, `https://claude.ai`), длина task ≤ 8000 символов, `maxTokens` 1–3000 (default 900).
- Контекст компилируется **тем же компилятором** `context::compile_context`, что и MCP (SESSION-08): список сущностей через `db::list_souls` + `db::list_entities`, только первая душа, текстовый запрос = текст из поля ввода веб-чата.
- Disclosure-квитанция пишется **до** ответа через `package::write_disclosure_receipt` с `client = "browser-companion:{origin}"` и **не содержит** текста задачи, id сущностей, claim/evidence — только метаданные (kind, at, client, entity_count, token_estimate, policy_version, state_version, max_tokens). Пак не пишется в логи/строки/квитанции (stderr — только фатальные ошибки).
- `open_app_db` — как в MCP: READ_ONLY с фолбэком на READ_WRITE (WAL без `-shm`); каталог через `resolve_app_dir` (env `SOUL_APP_DIR` → `%APPDATA%/ai.soul.runtime`).
- **~25 тестов**: кадры (пустые, битые, oversized), валидация протокола/ID/nonce/origin/task/maxTokens, replay, receipt-контент, пустая БД, отсутствующая БД, serve-цикл roundtrip, E2E против реального бинаря `soul-bridge` (spawn, кадры по stdin/stdout, завершение по EOF).

### Rust: регистрация host-а `src-tauri/src/native_host.rs` (новый)

- Манифест host-а `com.soul.browser_companion.json`: `{name, description, path: <binary>, type: "stdio", allowed_origins: ["chrome-extension://<ID>/"]}`, атомарная запись temp+rename.
- Windows: ключи `HKCU\Software\Google\Chrome\NativeMessagingHosts` и `HKCU\Software\Microsoft\Edge\NativeMessagingHosts` через `reg.exe`; unregister идемпотентен (нет ключа — не ошибка); binary — `soul-bridge(.exe)` рядом с текущим исполняемым файлом.
- `BridgeStatus {host_name, registered, manifest_exists, binary_exists, extension_id, error}`; манифест без ключей реестра — `registered: false` + `error`.
- Внедрение `CommandRunner` (closure через `Arc<Mutex<Vec<Vec<String>>>>`) — тесты не трогают реальный реестр. 10 тестов: содержимое манифеста, allowed_origins, register/unregister (win), идемпотентность, status-отражение, orphan-манифест.

### Rust: командный слой `src-tauri/src/lib.rs` + `src-tauri/src/bin/soul_bridge.rs` (новые)

- `pub mod bridge; mod native_host;`; команды `register_bridge_cmd`, `unregister_bridge_cmd`, `bridge_status_cmd` (все с `tauri::AppHandle` + `app_data_dir`) зарегистрированы в invoke_handler.
- `soul_bridge.rs`: `resolve_app_dir` → `serve_native_messaging`; ошибки в stderr, exit 1. Бинарь `soul-bridge` в `Cargo.toml`.

### Browser Companion: расширение MV3 (`browser/`)

- **`manifest.source.json`**: MV3, `"key"` — фиксированный RSA-2048 (генерируется один раз скриптом и переиспользуется), из него Chrome вычисляет стабильный ID `epfbcmgajbpjbphepfbhcoibmoaflbld`. Разрешения: только `nativeMessaging`; host_permissions и content_scripts — ровно 3 поддерживаемых домена. Без storage/tabs/cookies/popup/внешних ключей.
- **`scripts/make-extension-id.mjs`**: детерминированный генератор ключа (сохраняет ключ в `browser/keys/`, вычисляет ID из SHA-256(DER SPKI) → алфавит a-p).
- **`build.mjs`**: programmatic Vite (без новых зависимостей), IIFE-бандлы `background.js` и `content.js` в `browser/extension/` + копирование манифеста и иконок (16/32/48/128 из app-иконки).
- **`src/protocol.ts`**: типы `soul.ping`/`soul.get_context`/`soul.pong`/`soul.context`/`soul.error`; `validateOutgoingRequest` — зеркальная fail-closed валидация (протокол, ID, nonce, origin, task ≤ 8000, maxTokens 1–3000, размер кадра ≤ 1 МБ) до отправки в host.
- **`src/nonce.ts`**: `createNonce()` — 24 байта `crypto.getRandomValues` → base64url (32 символа); `isValidNonce`.
- **`src/compose.ts`**: блок `[SOUL context] … [/SOUL context]`, `composeMessage` (черновик + блок), `collapseText` (выделение текста пользователя/блока/остатка из сообщения истории), `chipLabel`, `itemCountFromPack`.
- **`src/adapters/{types,chatgpt,gemini,claude}.ts`**: версионированные адаптеры (`chatgpt/v1`, `gemini/v1`, `claude/v1`) с селекторами поля ввода/кнопки Send/точки монтажа чипа/контейнера истории; `probe()` — health check по обязательным селекторам (fail-closed при изменении разметки); `isSendEvent` — Enter без модификаторов и не IME, клик по кнопке Send.
- **`src/registry.ts`**: выбор адаптера по origin.
- **`src/dom.ts`**: мост Page/PageElement ↔ DOM; textarea — нативный сеттер + input/change (React-совместимо); contenteditable — focus + `execCommand('insertText')` (ProseMirror/Quill).
- **`src/ui/chip.ts`**: чип `SOUL ON/OFF/ERR/…` с shadow-root: точка состояния, число `· N items`, выключатель «1 msg» (пропустить контекст для следующего сообщения), аварийный выключатель «×» (сессия off до перезагрузки).
- **`src/background.ts`**: service worker; `connectNative(HOST_NAME)`, маршрутизация по nonce, таймаут 10 c, `onDisconnect` разворачивает ожидающие запросы в ошибку; контекст нигде не сохраняется.
- **`src/content.ts`**: перехват Enter/клика по Send (capture, `stopImmediatePropagation`), пауза текущей отправки, `soul.get_context` с текстом поля ввода, вставка собранного сообщения, автоматическое продолжение **той же** отправки (клик по кнопке Send, фолбэк Enter в поле) — без второго клика; busy-подавление повторных Send; fail-open при ошибке host-а (сообщение уходит без контекста); «1 msg» и «×»; health check каждые 30 с → fail-closed; MutationObserver + периодический sweep сворачивают блок в истории в `<details>`-чип `SOUL context: N items` (raw-блок сохраняется внутри чипа, текст пользователя остаётся).

### TypeScript: модель и UI

- **`src/data/bridge.ts` (новый)**: `BridgeStatus`, `bridgeStateLabel`, `bridgeStatusNote`, `COMPANION_SITES`.
- **`src/pages/Settings.tsx`**: секция «Browser Companion» — статус host-а (Register/Unregister/Check), бинарь, ID расширения, заметка о fail-closed и установке расширения из `browser/extension`.
- **`package.json`**: скрипт `build:companion`; lint расширен на `browser/src/`; `tsconfig.json` include `browser/src`.

## Изменённые файлы

- `src-tauri/src/bridge.rs` — новый (host, протокол, валидация, контекст, квитанция, тесты).
- `src-tauri/src/native_host.rs` — новый (манифест, reg.exe, статус, тесты).
- `src-tauri/src/bin/soul_bridge.rs` — новый (entry point host-а).
- `src-tauri/src/lib.rs` — `pub mod bridge; mod native_host;` + 3 команды companion.
- `src-tauri/Cargo.toml` — `[[bin]] soul-bridge`.
- `browser/manifest.source.json`, `browser/build.mjs`, `browser/scripts/make-extension-id.mjs`, `browser/icons/{16,32,48,128}.png` — новые (сборка расширения).
- `browser/src/*` — новые (protocol, nonce, compose, adapters, registry, dom, background, content, chip, chrome.d.ts, constants).
- `src/data/bridge.ts` — новый (модель для UI).
- `src/pages/Settings.tsx` — секция «Browser Companion».
- `package.json` — `build:companion`; lint `browser/src/`; `tsconfig.json` — include `browser/src`.
- `tests/browser-{protocol,nonce,compose,adapters,manifest}.test.ts` — новые (14 тестов).
- `.gitignore` — `browser/extension/` (артефакт сборки не коммитим).
- `README.md` — команда `pnpm build:companion`.

## Изменённые контракты

- Новый контракт: **soul-bridge/1** — Native Messaging фреймы (u32 LE + JSON), запросы `soul.ping`/`soul.get_context`, ответы `soul.pong`/`soul.context`/`soul.error`; лимиты 1 МБ кадр / 8000 символов task / maxTokens 1–3000; nonce 16–64 `[A-Za-z0-9_-]` + replay-защита; host `com.soul.browser_companion` с `allowed_origins` = ровно ID расширения.
- Схема SQLite не менялась; миграций нет.

## Запущенные тесты

- `cargo test --lib` (src-tauri): **PASS** — 128 passed (включая E2E реального бинаря `soul-bridge`).
- `cargo clippy --all-targets`: **PASS** — без предупреждений.
- `pnpm test`: **PASS** — 176 passed (включая 14 новых browser-тестов).
- `pnpm typecheck`: **PASS**.
- `pnpm lint`: **PASS**.
- `pnpm format`: **PASS**.
- `pnpm build`: **PASS**.
- `pnpm build:companion`: **PASS** — `browser/extension/{background.js, content.js, manifest.json, icons/}`.

## Проверка безопасности

- **Расширение не читает лишнего**: permissions ровно `nativeMessaging`; content_scripts/host_permissions — только 3 домена; нет storage, tabs, cookies, истории, popup, `externally_connectable`, oauth.
- **Host принимает только своё расширение**: `allowed_origins` = `chrome-extension://epfbcmgajbpjbphepfbhcoibmoaflbld/`, extension ID сверяется и на уровне протокола; тест-оверрайд через env только для юнит-тестов.
- **Подделка запроса из веб-страницы невозможна**: страницы не могут вызвать `chrome.runtime.sendMessage` в расширение с чужим содержимым сверх того, что объявлено в content script; любое сообщение валидируется (протокол, ID, nonce-формат, origin из строгого списка, размер).
- **Replay-защита**: повторный nonce в рамках сессии host-а отклоняется (`replay_detected`).
- **Контекст нигде не сохраняется**: нет storage; пак не попадает в квитанцию (только метаданные), в логи и crash-репорты; квитанция — локальный файл с client `browser-companion:{origin}`, entity_count, token_estimate и версиями.
- **Fail-closed**: неизвестная/изменённая разметка → `probe()` failed → адаптер и перехват отключаются (тест на устаревший селектор); без адаптера расширение не вмешивается в страницу.
- **Расширение не инициирует отправку само**: перехват только после действия пользователя (Enter/клик); программный клик по Send — только продолжение перехваченной отправки (resuming-флаг исключает петлю).
- **Отключение host-а**: `unregister_bridge_cmd` удаляет манифест и ключи; extension без host-а не получает контекст и не изменяет поведение чата (fail-open на уровне отправки).
- **Лимиты**: кадр 1 МБ, task 8000 символов, maxTokens 3000 — исключают злоупотребление размером; oversized-кадр → `request_too_large`.

## Влияние на производительность и токены

- Нативная отправка: 1 запрос к host-у за сообщение; компиляция использует тот же бюджет токенов, что MCP (default 900, hard cap 3000).
- Расширение в idle: health-probe раз в 30 с + sweep каждые 4 с по текстовым узлам контейнера истории (лимит 400 узлов за проход) — пренебрежимо мало.
- Пак вставляется в сообщение и после отправки сворачивается в чип; видимый пользователю текст не меняется (черновик + блок, блок скрывается в `<details>`).

## Известные ограничения

- Селекторы разметки зафиксированы на дату сессии и проверены юнит-тестами на фиктивной разметке, а **не на живых сайтах**: реальный ChatGPT/Gemini/Claude могут изменить DOM — тогда health check отключит адаптер (fail-closed), чип покажет `SOUL ERR`, и потребуется только bump версии адаптера. Ручная проверка в браузере — отдельный шаг после установки.
- `document.execCommand('insertText')` — deprecated API, но единственный надёжный способ обновить ProseMirror/Quill; при отключении в Chrome потребуется альтернатива.
- IME-ввод и некоторые раскладки: Enter при композиции не перехватывается (намеренно).
- E2E тест host-а использует реальный бинарь; для запуска нужен `cargo build --bins` (иначе тест пропускается).
- Ключ расширения — публичный RSA (как требует Chrome); секретным является только приватная часть, которая не хранится в репозитории.

## Коммит

- `9fcca2a` `feat(soul): browser companion extension for ChatGPT, Gemini and Claude web chats [session-09]`
