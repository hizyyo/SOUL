# CONTRIBUTING

## Протокол сессий

Каждая сессия разработки следует протоколу:

1. **Задачи** — определены в промпте сессии.
2. **Реализация** — строго в рамках P0 из `SOUL_ULTRA_MVP_CONTEXT.md`.
3. **Завершение** — каждая сессия заканчивается:
   - модульными тестами;
   - проверкой безопасности;
   - журналом сессии в `docs/session-log/`;
   - Git-коммитом.

## Журнал сессии

Файл: `docs/session-log/SESSION-XX.md`

Структура:
- Цель сессии
- Что сделано
- Что работает
- Тесты и проверки
- Ограничения и риски
- Хеш и message коммита

## Разработка

```bash
pnpm install
pnpm dev          # Tauri desktop
pnpm test         # Vitest
pnpm typecheck    # tsc --noEmit
pnpm lint         # ESLint
pnpm format       # Prettier check
```
