# SOUL

Personal Intelligence Runtime.

Локальная среда выполнения для принадлежащих пользователю решений, предпочтений, границ и полномочий ИИ-агентов.

## Текущая область P0

- Создание локального SOUL — готово (SESSION-01)
- Интерактивная калибровка (5 минут) — готово (SESSION-02)
- Типизированные сущности (preference, decision, boundary, goal, fact) — готово (SESSION-01/02)
- Локальный SQLite + FTS5 — готово (SESSION-01/07)
- Компилятор контекста — готово (SESSION-07)
- MCP-адаптер для coding-клиентов — готово (SESSION-08)
- Browser Companion для ChatGPT/Gemini/Claude Web — готово (SESSION-09)
- Blind Preference Test (20 раундов) — готово (SESSION-10)
- Детерминированный DSL политик — в плане
- Имитированный Gateway — в плане
- Экспорт и удаление — готово (SESSION-03/06)

## Локальная настройка

```bash
# Установка зависимостей
pnpm install

# Запуск desktop-приложения (Tauri 2)
pnpm dev

# Запуск тестов
pnpm test

# Проверка типов
pnpm typecheck

# Линтинг
pnpm lint

# Сборка расширения Browser Companion (результат в browser/extension/)
pnpm build:companion
```

## Стек

| Компонент | Технология |
|-----------|-----------|
| Desktop оболочка | Tauri 2 |
| Язык бэкенда | Rust |
| Фронтенд | React + TypeScript + Vite |
| Стилизация | Tailwind CSS (планируется) |
| База данных | SQLite + FTS5 |
| Валидация | Zod |

## Лицензия

Проприетарная.
