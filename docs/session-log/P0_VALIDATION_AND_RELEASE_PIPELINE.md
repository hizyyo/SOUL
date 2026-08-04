# P0-валидация и release pipeline

## Сделано

- Добавлен `docs/validation/P0_VALIDATION_PLAYBOOK.md`: набор внешней P0-валидации без имитации результатов - рекрутинг 12-15 внешних участников, consent и минимизация данных, 30-минутная сессия, Blind Preference Test против B1, интервью, willingness-to-pay, day-7/day-28 retention, kill/pivot criteria и решение основателя.
- Добавлен `docs/validation/P0_VALIDATION_REPORT.md`: единственный шаблон статуса гейта. Исходный статус - `BLOCKED`; шаблон прямо запрещает подменять внешние данные внутренними тестами, synthetic data или выдуманными метриками.
- `SOUL_ULTRA_MVP_CONTEXT.md` теперь требует читать playbook/report/readiness перед SESSION-15. Биллинг/P1 остаются заблокированными до реальных внешних участников, отчёта, защищённых первичных данных, day-7/day-28 retention и решения основателя `CONTINUE_TO_P1`.
- Найдена причина release failure: пользовательский `~/.cargo/config.toml` направлял target `x86_64-pc-windows-msvc` на LLVM-MinGW `lld-link`, который несовместим с vendored OpenSSL. Добавлен `scripts/release-check.ps1` и `pnpm release:check`; он поднимает MSVC Build Tools, выбирает настоящий `link.exe` и переопределяет linker только для процесса проверки.
- Добавлены release benchmarks: локальная policy p95 < 5 ms; cold context p95 < 75 ms на 1 000 сущностей; cached context p95 < 75 ms на 10 000 сущностей. Cold 10k остался отдельным regression test без ложного обещания 75 ms для полного пересчёта.

## Результаты release

`pnpm release:check` (Windows/MSVC):

- policy p95: **12 µs**;
- cold context p95, 1 000 entities: **8.7759 ms**;
- cached context p95, 10 000 entities: **7.4 µs**.

В linker output остаются предупреждения `LNK4099` об отсутствующем debug PDB vendored OpenSSL; release build и тесты успешно завершаются, на результат runtime это не влияет.

## Честное ограничение

Реальная P0-валидация ещё не проведена: её невозможно выполнить из репозитория без доступа к внешним людям. В проект добавлен исполнимый процесс, но `P0_VALIDATION_REPORT.md` остаётся `BLOCKED` до фактического набора, интервью и retention follow-ups.
