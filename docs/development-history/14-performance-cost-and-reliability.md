# Performance, Cost, and Reliability

## Цель

Сделать P0 быстрым, дешёвым и устойчивым: убрать повторную работу при сборке контекста и импорте, измерять размер контекста и его оценочную стоимость, ограничить выдачу больших списков и закрепить регрессии тестами.

## Контекст и стоимость модели

- SOUL не вызывает модель и не отправляет данные в сеть. Контекст компилируется детерминированно локально; токены означают входной размер context pack, который пользовательский MCP-клиент или Browser Companion добавляет во внешний чат.
- Добавлена консервативная оценка стоимости входа: `COST_USD_PER_1K_INPUT_TOKENS = $0.005`. Это не биллинг и не фактический тариф провайдера, а прозрачная локальная оценка.
- `DisclosureReceipt`, MCP metadata и bridge response теперь содержат `costEstimateUsd`; Settings показывает стоимость каждой выдачи и агрегат `ContextUsageStats` (вызовы, суммарные input tokens, оценочная стоимость, последнее раскрытие).

## Кеши и идемпотентность

- **Кеш контекста**: `compile_context_cached` хранит последний pack по пути БД, ревизиям состояния/политик и каноническому fingerprint запроса. `state_revision` и `policy_revision` меняются в той же транзакции, что и сущность/политика/импорт, поэтому старый pack не переживает мутацию.
- **Кеш извлечения по hash содержимого**: пакет полностью проверяется (формат, подпись, KDF, hash, event chain, entities) при каждом импорте. Если `content_hash`, `soul_id` и ревизия совпадают с последним импортом, данные не wipe/rewrite. Повторный импорт не порождает модельных токенов по построению и не выполняет лишнюю пересборку БД.
- Тесты подтверждают fast path и корректный переход на полный импорт после локальной мутации.

## Производительность и надёжность

- Контекстный hot path больше не парсит JSON сущности многократно: `ParsedEntity` извлекает claim/evidence/scope/sensitivity/domains/confidence одним парсингом и переиспользует их в фильтрах, релевантности и пакете.
- Термы запроса токенизируются один раз; конфликтные и superseded-группы используют HashMap-индексы вместо повторного линейного поиска.
- Упаковка пакета считает токены инкрементально целочисленной моделью, сохраняя семантику прежней оценки и не пересериализуя весь pack для каждой кандидатной сущности.
- Полнотекстовый поиск читает `limit + 1`: Tauri-ответ `SearchResult` и Context UI теперь честно показывают `truncated`, а не создают видимость полного списка.
- Добавлен regression test для компиляции 10 000 сущностей. Debug-порог 5 s намеренно защищает от квадратичной деградации при параллельном запуске тестов; продукционный критерий относится к cached Rust пути в оптимизированной сборке.
- TS parity-preview тест измеряет p95, имеет warmup и использует 200 ms smoke-порог: интерфейсная TS-копия не является production path, а строгая цель P0 (`p95 < 75 ms`) относится к Rust cached path.

## Проверки

- `cargo test --lib`: **PASS** — 229 passed.
- `cargo clippy --all-targets -- -D warnings`: **PASS** — 0 warnings.
- `cargo fmt --check`: **PASS**.
- `pnpm test`: **PASS** — 235 passed.
- `pnpm exec tsc --noEmit`: **PASS**.
- `pnpm lint`: **PASS**.
- `pnpm format`: **PASS**.
- `pnpm release:check`: **PASS** на Windows/MSVC. Глобальный LLVM-MinGW `lld-link` оказался несовместим с vendored OpenSSL; project script поднимает MSVC Build Tools и локально переопределяет linker. Измерения release: policy p95 **12 µs**, cold context p95 на 1 000 сущностей **8.7759 ms**, cached context p95 на 10 000 сущностей **7.4 µs**.

## Изменённые файлы

- `src-tauri/src/context.rs` — кеш контекста, fingerprint, ревизии, оценка стоимости, parse-once hot path, инкрементальная упаковка, 10k regression tests.
- `src-tauri/src/db.rs`, `src-tauri/src/policy.rs` — meta revisions и invalidation при мутациях; FTS `limit + 1` + `truncated`.
- `src-tauri/src/package.rs` — content-hash import fast path, metadанные импорта, стоимость в квитанциях, usage aggregate и тесты.
- `src-tauri/src/mcp.rs`, `src-tauri/src/bridge.rs`, `src-tauri/src/lib.rs` — cached compiler, стоимость в API и команда `context_usage_cmd`; typed `SearchResult`.
- `src/data/context.ts`, `src/pages/Context.tsx`, `src/pages/Settings.tsx` — показ оценки стоимости, усечённых FTS-результатов и статистики раскрытий.
- `tests/context.test.ts` — p95-based UI preview perf regression with warmup.

## Оставшийся production gate

1. Не включать биллинг или production distribution до kill criteria, внешнего P0-тестирования и явного решения основателя (см. `docs/PRODUCTION_READINESS.md`).

## Коммит

- `f86c7ae` `perf(soul): session-14 context cache, import dedup and usage metrics [session-14]`
