# SOUL

Personal Intelligence Runtime.

Локальная среда выполнения для принадлежащих пользователю решений, предпочтений, границ и полномочий ИИ-агентов.

## Текущая область P0

- Создание локального SOUL
- Интерактивная калибровка (5 минут)
- Типизированные сущности (preference, decision, boundary, goal, fact)
- Локальный SQLite + FTS5
- Компилятор контекста
- MCP-адаптер для coding-клиентов
- Browser Companion для ChatGPT/Gemini/Claude Web
- Blind Preference Test (20 раундов)
- Детерминированный DSL политик
- Имитированный Gateway
- Экспорт и удаление

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
