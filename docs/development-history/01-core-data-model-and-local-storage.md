# Core Data Model and Local Storage

**Дата:** 2026-07-31

## Цель

Реализовать ядро данных SOUL: типизированные сущности, локальное SQLite-хранилище и событийный event log.

## Что сделано

- Создан workspace-пакет `packages/soul-schema/` с Zod-схемами всех P0-сущностей (preference, decision, boundary, goal, fact)
- Добавлен `pnpm-workspace.yaml` для монорепозитория
- Добавлена SQLite-база (`rusqlite` bundled) в Rust-бэкенд
- Реализован event store с криптографической цепочкой хешей (SHA-256)
- Реализованы Tauri-команды: `init_app`, `create_soul_cmd`, `get_soul_cmd`, `add_entity_cmd`, `list_entities_cmd`
- Обновлён фронтенд: экран создания SOUL (форма display name), отображение статуса и списка сущностей
- Добавлены типы: EntityStatus, Sensitivity, Stability, EntityScope — полное описание из SOUL_MASTER_PLAN.md 7.2
- Добавлены тесты: валидация Zod-схем (10 тестов), health-тест

## Что работает

- `pnpm typecheck` — строгая проверка TS
- `pnpm lint` — ESLint без ошибок
- `pnpm test` — 12 тестов проходят
- `cargo check` — Rust/Tauri компилируется без ошибок и предупреждений
- SQLite инициализируется с WAL-режимом и foreign keys
- Event store: soul.created, candidate.proposed, entity.activated
- SHA-256 хеширование для цепочки событий
- Фронтенд: создание SOUL через форму, отображение сущностей

## Тесты и проверки

| Проверка | Статус |
|----------|--------|
| typecheck (tsc --noEmit) | Пройден |
| lint (ESLint) | Пройден |
| schema validation (Vitest, 10 tests) | Пройден |
| health test (Vitest, 2 tests) | Пройден |
| Rust build (cargo check) | Пройден |
| Tauri commands CRUD | Реализованы |

## Ограничения и риски

- Нет шифрования SQLite (будет добавлено в SESSION-02 вместе с crypto-ключом)
- Нет миграций схемы БД (пока создаются IF NOT EXISTS)
- Фронтенд минимальный — только создание SOUL и список сущностей
- Нет экспорта/импорта .soul-пакета
- Нет интерактивной калибровки (P0, будет в SESSION-02)
