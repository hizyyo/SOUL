use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SoulManifest {
    pub soul_id: String,
    pub display_name: String,
    pub format_version: String,
    pub schema_version: String,
    pub created_at: String,
    pub head_event_hash: Option<String>,
    pub entity_count: i64,
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct SoulEvent {
    pub event_id: String,
    pub soul_id: String,
    pub device_id: String,
    pub actor: String,
    pub hlc: String,
    pub operation: String,
    pub entity_type: String,
    pub entity_id: String,
    pub payload: String,
    pub provenance_ids: Vec<String>,
    pub previous_event_hash: Option<String>,
    pub content_hash: String,
    #[serde(default = "legacy_event_hash_version")]
    pub hash_version: i32,
    pub signature: String,
    pub created_at: String,
}

fn legacy_event_hash_version() -> i32 {
    1
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityRow {
    pub id: String,
    pub soul_id: String,
    pub entity_type: String,
    pub status: String,
    pub data: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn init_db(app_dir: &std::path::Path) -> SqlResult<Connection> {
    let db_path = app_dir.join("soul.db");
    let key_hex = hex::encode(crate::crypto::db_encryption_key(app_dir).map_err(to_sql_error)?);
    recover_interrupted_migration(app_dir, &db_path, &key_hex)?;

    match open_encrypted(&db_path, &key_hex) {
        Ok(conn) => init_schema(conn),
        Err(err) if is_not_a_database(&err) && db_path.exists() => {
            // Совместимость: БД создана до внедрения SQLCipher (plaintext).
            // Переносим в зашифрованный файл и повторяем инициализацию.
            match migrate_plaintext_to_encrypted(app_dir, &db_path, &key_hex) {
                Ok(conn) => init_schema(conn),
                Err(migrate_err) if is_not_a_database(&migrate_err) => Err(to_sql_error(format!(
                    "SOUL database is unreadable with the current device key and is not a valid plaintext database; refusing to delete it: {migrate_err}"
                ))),
                Err(other) => Err(other),
            }
        }
        Err(err) => Err(err),
    }
}

fn recover_interrupted_migration(
    app_dir: &std::path::Path,
    db_path: &std::path::Path,
    key_hex: &str,
) -> SqlResult<()> {
    if db_path.exists() {
        return Ok(());
    }
    let tmp_path = app_dir.join("soul.new");
    if tmp_path.exists() && open_encrypted(&tmp_path, key_hex).is_ok() {
        std::fs::rename(&tmp_path, db_path)
            .map_err(|e| to_sql_error(format!("Migration recovery failed: {e}")))?;
        let _ = std::fs::remove_file(app_dir.join("soul.db.bak"));
        return Ok(());
    }
    let bak_path = app_dir.join("soul.db.bak");
    if bak_path.exists() {
        std::fs::rename(&bak_path, db_path)
            .map_err(|e| to_sql_error(format!("Migration rollback recovery failed: {e}")))?;
    }
    Ok(())
}

/// Конвертирует строковую ошибку (например, от операций с файлами) в
/// rusqlite::Error, чтобы не менять сигнатуры публичных функций.
fn to_sql_error(msg: String) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ErrorCode::Unknown as i32),
        Some(msg),
    )
}

/// Открывает файл как зашифрованную БД SQLCipher и верифицирует ключ.
fn open_encrypted(db_path: &std::path::Path, key_hex: &str) -> SqlResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(conn)
}

/// Перенос данных из унаследованной plaintext-БД в новый зашифрованный файл.
/// Старый файл резервируется как `soul.db.bak` и удаляется после успеха.
fn migrate_plaintext_to_encrypted(
    app_dir: &std::path::Path,
    db_path: &std::path::Path,
    key_hex: &str,
) -> SqlResult<Connection> {
    let tmp_path = app_dir.join("soul.new");
    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)
            .map_err(|e| to_sql_error(format!("Cleanup failed: {e}")))?;
    }
    {
        let tmp = Connection::open(&tmp_path)?;
        tmp.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;
        // Keep migration self-contained in soul.new. WAL sidecars cannot be
        // atomically swapped with the main database file.
        tmp.execute_batch("PRAGMA journal_mode=DELETE;")?;
        tmp.execute_batch("PRAGMA foreign_keys=OFF;")?;
        tmp.execute_batch(BASE_SCHEMA_SQL)?;
        add_column_if_missing(&tmp, "entities", "dedup_key", "TEXT")?;
        add_column_if_missing(&tmp, "events", "hash_version", "INTEGER NOT NULL DEFAULT 1")?;
        add_column_if_missing(
            &tmp,
            "soul_state",
            "preview_confirmed",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        tmp.execute_batch(DEDUP_INDEX_SQL)?;

        // Create every logical table before copying. These initializers may
        // seed defaults; copy_logical_tables replaces seeded rows only when
        // the corresponding table exists in the plaintext source.
        crate::eval::init_evaluations(&tmp)?;
        crate::policy::init_policies(&tmp)?;
        crate::gateway::init_gateway(&tmp)?;

        let old = Connection::open(db_path)?;
        copy_logical_tables(&old, &tmp)?;

        // FTS создаётся после копирования: contentless-индекс пересобирается
        // по текущим сущностям (rowid сохраняется построчным переносом).
        init_fts(&tmp)?;
        tmp.execute_batch("PRAGMA foreign_keys=ON;")?;
    }

    let bak_path = app_dir.join("soul.db.bak");
    let _ = std::fs::remove_file(&bak_path);
    let _ = std::fs::remove_file(app_dir.join("soul.db-wal"));
    let _ = std::fs::remove_file(app_dir.join("soul.db-shm"));
    std::fs::rename(db_path, &bak_path).map_err(|e| to_sql_error(format!("Swap failed: {e}")))?;
    std::fs::rename(&tmp_path, db_path).map_err(|e| to_sql_error(format!("Swap failed: {e}")))?;
    let _ = std::fs::remove_file(&bak_path);

    open_encrypted(db_path, key_hex)
}

/// Only application-owned logical tables are copied. FTS virtual/shadow
/// tables are deliberately omitted and rebuilt from entities after the copy.
/// Rowids are preserved because entity_fts joins entities by rowid.
fn copy_logical_tables(old: &Connection, new: &Connection) -> SqlResult<()> {
    const LOGICAL_TABLES: [&str; 12] = [
        "souls",
        "events",
        "entities",
        "soul_state",
        "meta",
        "evaluations",
        "policies",
        "policy_meta",
        "capabilities",
        "gateway_receipts",
        "gateway_connectors",
        "gateway_meta",
    ];

    for table in LOGICAL_TABLES {
        let exists: bool = old.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1)",
            params![table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }

        let old_cols = table_columns(old, table)?;
        let new_cols = table_columns(new, table)?;
        let cols: Vec<String> = old_cols
            .into_iter()
            .filter(|column| new_cols.contains(column))
            .collect();
        if cols.is_empty() {
            continue;
        }
        new.execute(&format!("DELETE FROM \"{table}\""), [])?;
        let quoted: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
        let sel_sql = format!("SELECT rowid, {} FROM \"{table}\"", quoted.join(", "));
        let ins_sql = format!(
            "INSERT INTO \"{table}\" (rowid, {}) VALUES ({})",
            quoted.join(", "),
            (0..=cols.len()).map(|_| "?").collect::<Vec<_>>().join(", ")
        );
        let mut sel = old.prepare(&sel_sql)?;
        let mut vals = sel.query([])?;
        while let Some(row) = vals.next()? {
            let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(cols.len() + 1);
            params.push(rusqlite::types::Value::Integer(row.get::<_, i64>(0)?));
            for i in 0..cols.len() {
                params.push(row.get::<_, rusqlite::types::Value>(i + 1)?);
            }
            new.execute(&ins_sql, rusqlite::params_from_iter(params.iter()))?;
        }
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> SqlResult<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::NotADatabase
    )
}

fn init_schema(conn: Connection) -> SqlResult<Connection> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    conn.execute_batch(BASE_SCHEMA_SQL)?;

    add_column_if_missing(&conn, "entities", "dedup_key", "TEXT")?;
    add_column_if_missing(
        &conn,
        "events",
        "hash_version",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        &conn,
        "soul_state",
        "preview_confirmed",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    conn.execute_batch(DEDUP_INDEX_SQL)?;

    init_fts(&conn)?;

    crate::eval::init_evaluations(&conn)?;
    crate::policy::init_policies(&conn)?;
    crate::gateway::init_gateway(&conn)?;
    conn.execute(
        "DELETE FROM meta WHERE key IN (
            'imported_content_hash',
            'imported_soul_id',
            'imported_state_revision'
        )",
        [],
    )?;
    ensure_active_soul(&conn)?;

    Ok(conn)
}

/// Базовая схема приложения (SQLCipher). Используется и при создании новой
/// БД (init_schema), и при переносе унаследованной plaintext-БД.
const BASE_SCHEMA_SQL: &str = "
        CREATE TABLE IF NOT EXISTS souls (
            soul_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            format_version TEXT NOT NULL DEFAULT '0.1.0',
            schema_version TEXT NOT NULL DEFAULT '0.1.0',
            created_at TEXT NOT NULL,
            head_event_hash TEXT,
            entity_count INTEGER NOT NULL DEFAULT 0,
            device_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS events (
            event_id TEXT PRIMARY KEY,
            soul_id TEXT NOT NULL,
            device_id TEXT NOT NULL,
            actor TEXT NOT NULL,
            hlc TEXT NOT NULL,
            operation TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            provenance_ids TEXT NOT NULL DEFAULT '[]',
            previous_event_hash TEXT,
            content_hash TEXT NOT NULL,
            hash_version INTEGER NOT NULL DEFAULT 2,
            signature TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY,
            soul_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            data TEXT NOT NULL DEFAULT '{}',
            dedup_key TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE TABLE IF NOT EXISTS soul_state (
            soul_id TEXT PRIMARY KEY,
            activated INTEGER NOT NULL DEFAULT 0,
            calibration_step INTEGER NOT NULL DEFAULT 0,
            calibration_answers TEXT NOT NULL DEFAULT '[]',
            preview_confirmed INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_events_soul ON events(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_soul ON entities(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);";

const DEDUP_INDEX_SQL: &str = "CREATE UNIQUE INDEX IF NOT EXISTS idx_entities_dedup
         ON entities(soul_id, dedup_key) WHERE dedup_key IS NOT NULL;";

const FTS_CREATE_SQL: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS entity_fts USING fts5(
            id UNINDEXED,
            claim,
            evidence,
            entity_type UNINDEXED,
            status UNINDEXED,
            content='',
            tokenize='unicode61 remove_diacritics 2'
        );

        CREATE TRIGGER IF NOT EXISTS entity_fts_ai AFTER INSERT ON entities BEGIN
            INSERT INTO entity_fts(rowid, id, claim, evidence, entity_type, status)
            VALUES (
                new.rowid,
                new.id,
                coalesce(json_extract(new.data, '$.claim'), ''),
                coalesce(json_extract(new.data, '$.evidence'), ''),
                new.entity_type,
                new.status
            );
        END;

        CREATE TRIGGER IF NOT EXISTS entity_fts_ad AFTER DELETE ON entities BEGIN
            INSERT INTO entity_fts(entity_fts, rowid, id, claim, evidence, entity_type, status)
            VALUES (
                'delete',
                old.rowid,
                old.id,
                coalesce(json_extract(old.data, '$.claim'), ''),
                coalesce(json_extract(old.data, '$.evidence'), ''),
                old.entity_type,
                old.status
            );
        END;

        CREATE TRIGGER IF NOT EXISTS entity_fts_au
        AFTER UPDATE OF data, entity_type, status ON entities BEGIN
            INSERT INTO entity_fts(entity_fts, rowid, id, claim, evidence, entity_type, status)
            VALUES (
                'delete',
                old.rowid,
                old.id,
                coalesce(json_extract(old.data, '$.claim'), ''),
                coalesce(json_extract(old.data, '$.evidence'), ''),
                old.entity_type,
                old.status
            );
            INSERT INTO entity_fts(rowid, id, claim, evidence, entity_type, status)
            VALUES (
                new.rowid,
                new.id,
                coalesce(json_extract(new.data, '$.claim'), ''),
                coalesce(json_extract(new.data, '$.evidence'), ''),
                new.entity_type,
                new.status
            );
        END;";

/// Полнотекстовый индекс сущностей (SQLite FTS5, contentless).
/// claim/evidence берутся из JSON-колонки data через json_extract; синхронизация
/// поддерживается триггерами на все пути записи (add/update/import/wipe),
/// поэтому FTS никогда не расходится с таблицей entities. Contentless-таблица
/// выбрана потому, что внешний контент (content='entities') требует колонок
/// с именами FTS-колонок, а данные лежат в JSON.
fn init_fts(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(FTS_CREATE_SQL)?;

    // Backfill только для пустого индекса (старые базы, созданные до появления
    // FTS): contentless-таблица не поддерживает 'rebuild', заполняем вручную.
    let indexed: i64 = conn.query_row("SELECT count(*) FROM entity_fts", [], |row| row.get(0))?;
    if indexed == 0 {
        conn.execute(
            "INSERT INTO entity_fts(rowid, id, claim, evidence, entity_type, status)
             SELECT rowid, id,
                    coalesce(json_extract(data, '$.claim'), ''),
                    coalesce(json_extract(data, '$.evidence'), ''),
                    entity_type, status
             FROM entities",
            [],
        )?;
    }
    Ok(())
}

/// Превращает произвольный пользовательский текст в безопасный FTS5 MATCH-запрос:
/// непрерывные alphanumeric-последовательности (unicode) экранируются двойными
/// кавычками и связываются через AND. Подчёркивания и дефисы — разделители,
/// поэтому "topic_7" ищется как "topic" AND "7". Мусорные запросы (только
/// спецсимволы, пустота) дают None — пустой результат, а не ошибку.
pub fn fts_match_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\""))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

/// Полнотекстовый поиск сущностей по claim/evidence (FTS5 + bm25).
/// Результаты ограничены душой (soul_id) — чужие сущности не утекают.
/// Возвращает на одну строку больше лимита, чтобы вызвающий мог показать
/// `truncated: true` означает, что совпадений строго больше, чем показано.
pub fn search_entities(
    conn: &Connection,
    soul_id: &str,
    query: &str,
    limit: usize,
) -> SqlResult<(Vec<EntityRow>, bool)> {
    let limit = limit.clamp(1, 100);
    let Some(match_expr) = fts_match_query(query) else {
        return Ok((Vec::new(), false));
    };
    let mut stmt = conn.prepare(
        "SELECT e.id, e.soul_id, e.entity_type, e.status, e.data, e.created_at, e.updated_at
         FROM entity_fts
         JOIN entities e ON e.rowid = entity_fts.rowid
         WHERE entity_fts MATCH ?1 AND e.soul_id = ?2
         ORDER BY bm25(entity_fts) ASC, e.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![match_expr, soul_id, limit as i64 + 1], |row| {
        Ok(EntityRow {
            id: row.get(0)?,
            soul_id: row.get(1)?,
            entity_type: row.get(2)?,
            status: row.get(3)?,
            data: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    let truncated = result.len() > limit;
    if truncated {
        result.truncate(limit);
    }
    Ok((result, truncated))
}

/// Аддитивная миграция: добавляет колонку, только если её ещё нет.
/// Позволяет открывать базы, созданные предыдущими версиями приложения.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, ddl: &str) -> SqlResult<()> {
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}");
    match conn.execute(&sql, []) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(_, Some(msg)))
            if msg.contains("duplicate column name") =>
        {
            Ok(())
        }
        Err(other) => Err(other),
    }
}

pub fn compute_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn get_soul_state(conn: &Connection, soul_id: &str) -> SqlResult<(i32, String, bool, bool)> {
    let mut stmt = conn.prepare(
        "SELECT calibration_step, calibration_answers, activated, preview_confirmed FROM soul_state WHERE soul_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![soul_id], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)? != 0,
            row.get::<_, i32>(3)? != 0,
        ))
    })?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok((0, "[]".to_string(), false, false)),
    }
}

const WIPE_SQL: &str = "DELETE FROM entities;
     DELETE FROM events;
     DELETE FROM soul_state;
     DELETE FROM evaluations;
     DELETE FROM policies;
     DELETE FROM policy_meta;
     DELETE FROM capabilities;
     DELETE FROM gateway_receipts;
     DELETE FROM gateway_connectors;
     DELETE FROM gateway_meta;
     DELETE FROM souls;
     DELETE FROM meta WHERE key NOT IN ('state_revision', 'policy_revision');";

pub fn wipe_all(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("PRAGMA secure_delete=ON;")?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    conn.execute_batch(WIPE_SQL)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    conn.execute_batch("VACUUM;")?;
    bump_revision(conn, META_STATE_REVISION)?;
    bump_revision(conn, META_POLICY_REVISION)?;
    Ok(())
}

/// Удаляет все данные без checkpoint/VACUUM. Вызывается изнутри активной
/// транзакции (например, при импорте пакета), чтобы откат ошибки в той же
/// транзакции не терял существующие данные.
pub fn wipe_all_tx(tx: &Transaction<'_>) -> SqlResult<()> {
    tx.execute_batch(WIPE_SQL)?;
    bump_revision(tx, META_STATE_REVISION)?;
    bump_revision(tx, META_POLICY_REVISION)?;
    Ok(())
}

/// Ключи таблицы `meta`: монотонные ревизии состояния и политик
/// для кеша контекста по версии. Ревизии никогда не сбрасываются (wipe их
/// увеличивает), поэтому ключ кеша не может случайно совпасть после wipe.
pub const META_STATE_REVISION: &str = "state_revision";
pub const META_POLICY_REVISION: &str = "policy_revision";
pub const META_ACTIVE_SOUL_ID: &str = "active_soul_id";

pub fn get_meta(conn: &Connection, key: &str) -> SqlResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
    let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn active_soul_id(conn: &Connection) -> Result<Option<String>, String> {
    let Some(id) = get_meta(conn, META_ACTIVE_SOUL_ID).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    if get_soul(conn, &id).map_err(|e| e.to_string())?.is_none() {
        return Err("The configured active SOUL does not exist.".to_string());
    }
    Ok(Some(id))
}

pub fn set_active_soul(conn: &Connection, soul_id: &str) -> Result<(), String> {
    if get_soul(conn, soul_id)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("SOUL not found.".to_string());
    }
    set_meta(conn, META_ACTIVE_SOUL_ID, soul_id).map_err(|e| e.to_string())?;
    bump_state_revision(conn).map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_active_soul(conn: &Connection) -> SqlResult<()> {
    if let Some(active_id) = get_meta(conn, META_ACTIVE_SOUL_ID)? {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM souls WHERE soul_id = ?1)",
            params![active_id],
            |row| row.get(0),
        )?;
        if exists {
            return Ok(());
        }
        conn.execute(
            "DELETE FROM meta WHERE key = ?1",
            params![META_ACTIVE_SOUL_ID],
        )?;
    }
    let mut stmt = conn.prepare("SELECT soul_id FROM souls ORDER BY created_at ASC LIMIT 2")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if ids.len() == 1 {
        set_meta(conn, META_ACTIVE_SOUL_ID, &ids[0])?;
    }
    Ok(())
}

/// Монотонный счётчик: читает, увеличивает, записывает. Внутри транзакции
/// (add_entity/update_entity/activate_preview/импорт) — атомарно с мутацией.
fn bump_revision(conn: &Connection, key: &str) -> SqlResult<i64> {
    let current: i64 = get_meta(conn, key)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let next = current + 1;
    set_meta(conn, key, &next.to_string())?;
    Ok(next)
}

pub fn bump_state_revision(conn: &Connection) -> SqlResult<i64> {
    bump_revision(conn, META_STATE_REVISION)
}

pub fn bump_policy_revision(conn: &Connection) -> SqlResult<i64> {
    bump_revision(conn, META_POLICY_REVISION)
}

/// Текущие ревизии для ключа кеша контекста. Свежая БД (без записей) — 0.
pub fn state_revision(conn: &Connection) -> SqlResult<i64> {
    Ok(get_meta(conn, META_STATE_REVISION)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

pub fn policy_revision(conn: &Connection) -> SqlResult<i64> {
    Ok(get_meta(conn, META_POLICY_REVISION)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

pub fn get_calibration(conn: &Connection, soul_id: &str) -> SqlResult<(i32, String)> {
    let mut stmt = conn.prepare(
        "SELECT calibration_step, calibration_answers FROM soul_state WHERE soul_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![soul_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
    })?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok((0, "[]".to_string())),
    }
}

pub fn save_calibration(
    conn: &Connection,
    soul_id: &str,
    step: i32,
    answers: &str,
) -> SqlResult<()> {
    if !(0..=100).contains(&step) || answers.len() > 256 * 1024 {
        return Err(to_sql_error(
            "Calibration payload exceeds allowed limits.".to_string(),
        ));
    }
    conn.execute(
        "INSERT INTO soul_state (soul_id, calibration_step, calibration_answers)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(soul_id) DO UPDATE SET
           calibration_step = excluded.calibration_step,
           calibration_answers = excluded.calibration_answers",
        params![soul_id, step, answers],
    )?;
    conn.execute(
        "UPDATE soul_state SET preview_confirmed = 0 WHERE soul_id = ?1 AND activated = 0",
        params![soul_id],
    )?;
    Ok(())
}

/// Явное подтверждение пользователем предпросмотра начального SOUL.
/// Идемпотентно: повторный вызов не создаёт дублирующих событий.
pub fn confirm_soul_preview(
    conn: &Connection,
    soul_id: &str,
    device_id: &str,
) -> Result<(), String> {
    let (_, _, _, preview_confirmed) = get_soul_state(conn, soul_id).map_err(|e| e.to_string())?;
    if preview_confirmed {
        return Ok(());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO soul_state (soul_id, preview_confirmed)
         VALUES (?1, 1)
         ON CONFLICT(soul_id) DO UPDATE SET preview_confirmed = 1",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    append_event(
        &tx,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation: "soul.preview_confirmed",
            entity_type: "fact",
            entity_id: soul_id,
            payload: &serde_json::json!({ "soulId": soul_id }),
        },
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Отмена подтверждения предпросмотра. Разрешена только до активации SOUL
/// (fail-closed: активированный SOUL сбросить нельзя). Идемпотентно.
pub fn reset_soul_preview(conn: &Connection, soul_id: &str, device_id: &str) -> Result<(), String> {
    let (_, _, activated, preview_confirmed) =
        get_soul_state(conn, soul_id).map_err(|e| e.to_string())?;
    if activated {
        return Err("Preview confirmation cannot be reset after activation.".to_string());
    }
    if !preview_confirmed {
        return Ok(());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE soul_state SET preview_confirmed = 0 WHERE soul_id = ?1",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    append_event(
        &tx,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation: "soul.preview_revoked",
            entity_type: "fact",
            entity_id: soul_id,
            payload: &serde_json::json!({ "soulId": soul_id }),
        },
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Активация SOUL возможна только после явного подтверждения предпросмотра.
/// Fail-closed: без preview_confirmed активация отклоняется.
pub fn activate_soul(conn: &Connection, soul_id: &str, device_id: &str) -> Result<(), String> {
    let (_, _, _, preview_confirmed) = get_soul_state(conn, soul_id).map_err(|e| e.to_string())?;
    if !preview_confirmed {
        return Err("SOUL cannot be activated before the preview is confirmed.".to_string());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    set_activated(&tx, soul_id, device_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn set_activated(conn: &Connection, soul_id: &str, device_id: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO soul_state (soul_id, activated)
         VALUES (?1, 1)
         ON CONFLICT(soul_id) DO UPDATE SET activated = 1",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    append_event(
        conn,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation: "soul.activated",
            entity_type: "fact",
            entity_id: soul_id,
            payload: &serde_json::json!({ "soulId": soul_id, "previewConfirmed": true }),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Массовая активация из предпросмотра: активирует только нечувствительные
/// кандидаты из переданного списка. Требует предварительно подтверждённый
/// предпросмотр (confirm_soul_preview). Границы, чувствительные и спорные
/// пункты не могут быть активированы массовым подтверждением (fail-closed).
pub fn activate_preview(
    conn: &Connection,
    soul_id: &str,
    entity_ids: &[String],
    device_id: &str,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let (_, _, _, preview_confirmed) = get_soul_state(&tx, soul_id).map_err(|e| e.to_string())?;
    if !preview_confirmed {
        return Err("SOUL cannot be activated before the preview is confirmed.".to_string());
    }
    if is_soul_activated(&tx, soul_id).map_err(|e| e.to_string())? {
        return Err("SOUL is already activated.".to_string());
    }

    for id in entity_ids {
        let entity = get_entity(&tx, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Entity {id} not found."))?;
        if entity.soul_id != soul_id {
            return Err(format!("Entity {id} does not belong to this SOUL."));
        }
        if entity.status == "active" {
            continue;
        }
        if entity.status != "candidate" {
            return Err(format!(
                "Entity {id} cannot be activated by preview confirmation (status {}).",
                entity.status
            ));
        }
        if !eligible_for_bulk_activation(&entity) {
            return Err(format!(
                "Entity {id} is a boundary, sensitive or disputed item and cannot be activated by preview confirmation."
            ));
        }
    }

    let now = Utc::now().to_rfc3339();
    for id in entity_ids {
        let entity = get_entity(&tx, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Entity {id} not found."))?;
        if entity.status != "candidate" {
            continue;
        }
        tx.execute(
            "UPDATE entities SET status = 'active', updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        let payload = serde_json::json!({ "entityId": id });
        append_event(
            &tx,
            &NewEvent {
                soul_id,
                device_id,
                actor: "user",
                operation: "entity.activated",
                entity_type: &entity.entity_type,
                entity_id: id,
                payload: &payload,
            },
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "INSERT INTO soul_state (soul_id, preview_confirmed, activated)
         VALUES (?1, 1, 1)
         ON CONFLICT(soul_id) DO UPDATE SET preview_confirmed = 1, activated = 1",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    let payload = serde_json::json!({
        "soulId": soul_id,
        "previewConfirmed": true,
        "activatedEntityIds": entity_ids
    });
    append_event(
        &tx,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation: "soul.activated",
            entity_type: "fact",
            entity_id: soul_id,
            payload: &payload,
        },
    )
    .map_err(|e| e.to_string())?;

    bump_state_revision(&tx).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn eligible_for_bulk_activation(entity: &EntityRow) -> bool {
    if entity.entity_type == "boundary" {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&entity.data) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    if obj.get("risk").and_then(|r| r.as_bool()) == Some(true) {
        return false;
    }
    let sensitivity = obj
        .get("sensitivity")
        .and_then(|s| s.as_str())
        .unwrap_or("internal");
    if matches!(sensitivity, "sensitive" | "restricted") {
        return false;
    }
    if obj.get("disputed").and_then(|d| d.as_bool()) == Some(true) {
        return false;
    }
    true
}

pub fn is_soul_activated(conn: &Connection, soul_id: &str) -> SqlResult<bool> {
    let mut stmt = conn.prepare("SELECT activated FROM soul_state WHERE soul_id = ?1")?;
    let mut rows = stmt.query_map(params![soul_id], |row| Ok(row.get::<_, i32>(0)? != 0))?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok(false),
    }
}

pub fn get_entity(conn: &Connection, entity_id: &str) -> SqlResult<Option<EntityRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, soul_id, entity_type, status, data, created_at, updated_at
         FROM entities WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![entity_id], |row| {
        Ok(EntityRow {
            id: row.get(0)?,
            soul_id: row.get(1)?,
            entity_type: row.get(2)?,
            status: row.get(3)?,
            data: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

const MAX_CLAIM_CHARS: usize = 2000;
pub const MAX_ENTITY_JSON_BYTES: usize = 64 * 1024;
pub const MAX_ENTITY_FIELD_CHARS: usize = 8_000;
pub const MAX_ENTITY_ARRAY_ITEMS: usize = 256;
pub const MAX_ENTITY_OBJECT_FIELDS: usize = 128;
pub const MAX_ENTITIES_PER_SOUL: usize = 10_000;
pub const MAX_EVENTS_PER_SOUL: usize = 50_000;
pub const MAX_LIST_ENTITIES: usize = 10_000;
pub const MAX_SEARCH_QUERY_CHARS: usize = 1_000;
pub const MAX_ID_CHARS: usize = 512;
pub const MAX_DISPLAY_NAME_CHARS: usize = 512;
pub const MAX_EVENT_FIELD_CHARS: usize = 512;
pub const MAX_EVENT_PAYLOAD_BYTES: usize = MAX_ENTITY_JSON_BYTES;

const P0_ENTITY_TYPES: [&str; 5] = ["preference", "decision", "boundary", "goal", "fact"];

/// Используется и при импорте пакета (package::verify_entities), поэтому
/// видимость crate-уровня.
pub(crate) fn validate_entity_type_value(entity_type: &str) -> Result<(), String> {
    if P0_ENTITY_TYPES.contains(&entity_type) {
        Ok(())
    } else {
        Err(format!("Unknown entity type: {entity_type}"))
    }
}

pub(crate) fn validate_status_value(status: &str) -> Result<(), String> {
    if matches!(status, "candidate" | "active" | "rejected") {
        Ok(())
    } else {
        Err(format!("Unknown entity status: {status}"))
    }
}

pub(crate) fn validate_entity_data_json(data: &str) -> Result<(), String> {
    if data.len() > MAX_ENTITY_JSON_BYTES {
        return Err(format!(
            "Entity data is too large (limit {MAX_ENTITY_JSON_BYTES} bytes)."
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|_| "Entity data must be valid JSON.".to_string())?;
    if !value.is_object() {
        return Err("Entity data must be a JSON object.".to_string());
    }
    fn validate_value(value: &serde_json::Value, depth: usize) -> Result<(), String> {
        if depth > 16 {
            return Err("Entity data is nested too deeply.".to_string());
        }
        match value {
            serde_json::Value::String(s) if s.chars().count() > MAX_ENTITY_FIELD_CHARS => {
                Err(format!(
                    "Entity string field is too long (limit {MAX_ENTITY_FIELD_CHARS} characters)."
                ))
            }
            serde_json::Value::Array(items) => {
                if items.len() > MAX_ENTITY_ARRAY_ITEMS {
                    return Err(format!(
                        "Entity array is too large (limit {MAX_ENTITY_ARRAY_ITEMS} items)."
                    ));
                }
                for item in items {
                    validate_value(item, depth + 1)?;
                }
                Ok(())
            }
            serde_json::Value::Object(fields) => {
                if fields.len() > MAX_ENTITY_OBJECT_FIELDS {
                    return Err(format!(
                        "Entity object has too many fields (limit {MAX_ENTITY_OBJECT_FIELDS})."
                    ));
                }
                for (key, item) in fields {
                    if key.chars().count() > 256 {
                        return Err("Entity field name is too long.".to_string());
                    }
                    validate_value(item, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    validate_value(&value, 0)?;
    if let Some(claim) = value.get("claim").and_then(|c| c.as_str()) {
        if claim.chars().count() > MAX_CLAIM_CHARS {
            return Err(format!(
                "Entity claim is too long (limit {MAX_CLAIM_CHARS} characters)."
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_soul_manifest_fields(
    soul_id: &str,
    display_name: &str,
    device_id: &str,
) -> Result<(), String> {
    for (name, value, max) in [
        ("soul id", soul_id, MAX_ID_CHARS),
        ("display name", display_name, MAX_DISPLAY_NAME_CHARS),
        ("device id", device_id, MAX_ID_CHARS),
    ] {
        if value.trim().is_empty() || value.chars().count() > max {
            return Err(format!("{name} is empty or too long."));
        }
    }
    Ok(())
}

pub(crate) fn validate_event_fields(event: &SoulEvent) -> Result<(), String> {
    for (name, value) in [
        ("event id", event.event_id.as_str()),
        ("soul id", event.soul_id.as_str()),
        ("device id", event.device_id.as_str()),
        ("actor", event.actor.as_str()),
        ("hlc", event.hlc.as_str()),
        ("operation", event.operation.as_str()),
        ("entity type", event.entity_type.as_str()),
        ("entity id", event.entity_id.as_str()),
        ("created at", event.created_at.as_str()),
    ] {
        if value.trim().is_empty() || value.chars().count() > MAX_EVENT_FIELD_CHARS {
            return Err(format!("Event {name} is empty or too long."));
        }
    }
    if event.payload.len() > MAX_EVENT_PAYLOAD_BYTES {
        return Err(format!(
            "Event payload is too large (limit {MAX_EVENT_PAYLOAD_BYTES} bytes)."
        ));
    }
    serde_json::from_str::<serde_json::Value>(&event.payload)
        .map_err(|_| "Event payload must be valid JSON.".to_string())?;
    if event.provenance_ids.len() > MAX_ENTITY_ARRAY_ITEMS
        || event
            .provenance_ids
            .iter()
            .any(|id| id.chars().count() > MAX_ID_CHARS)
    {
        return Err("Event provenance ids exceed allowed limits.".to_string());
    }
    if event
        .previous_event_hash
        .as_ref()
        .is_some_and(|hash| hash.len() != 64)
        || event.content_hash.len() != 64
    {
        return Err("Event hash has an invalid length.".to_string());
    }
    if !matches!(event.hash_version, 1 | 2) {
        return Err(format!(
            "Unsupported event hash version: {}.",
            event.hash_version
        ));
    }
    Ok(())
}

/// Version 2 hashes the immutable event record and the previous hash, rather
/// than hashing payload alone. This makes equal-payload events unique and
/// prevents relinking an event without invalidating its hash.
pub(crate) fn event_content_hash(event: &SoulEvent) -> String {
    match event.hash_version {
        1 => compute_hash(&event.payload),
        _ => compute_hash(
            &serde_json::json!({
                "eventId": event.event_id,
                "soulId": event.soul_id,
                "deviceId": event.device_id,
                "actor": event.actor,
                "hlc": event.hlc,
                "operation": event.operation,
                "entityType": event.entity_type,
                "entityId": event.entity_id,
                "payload": event.payload,
                "provenanceIds": event.provenance_ids,
                "previousEventHash": event.previous_event_hash,
                "createdAt": event.created_at,
            })
            .to_string(),
        ),
    }
}

fn read_soul_head(conn: &Connection, soul_id: &str) -> SqlResult<Option<String>> {
    conn.query_row(
        "SELECT head_event_hash FROM souls WHERE soul_id = ?1",
        params![soul_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

pub struct NewEvent<'a> {
    pub soul_id: &'a str,
    pub device_id: &'a str,
    pub actor: &'a str,
    pub operation: &'a str,
    pub entity_type: &'a str,
    pub entity_id: &'a str,
    pub payload: &'a serde_json::Value,
}

pub fn append_event(conn: &Connection, ev: &NewEvent) -> SqlResult<String> {
    let event_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE soul_id = ?1",
        params![ev.soul_id],
        |row| row.get(0),
    )?;
    if event_count >= MAX_EVENTS_PER_SOUL as i64 {
        return Err(to_sql_error(format!(
            "Too many events for this SOUL (limit {MAX_EVENTS_PER_SOUL})."
        )));
    }
    let event_id = format!("evt_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let previous = read_soul_head(conn, ev.soul_id)?;
    let payload = serde_json::to_string(ev.payload).unwrap();
    let mut event = SoulEvent {
        event_id,
        soul_id: ev.soul_id.to_string(),
        device_id: ev.device_id.to_string(),
        actor: ev.actor.to_string(),
        hlc: now.clone(),
        operation: ev.operation.to_string(),
        entity_type: ev.entity_type.to_string(),
        entity_id: ev.entity_id.to_string(),
        payload,
        provenance_ids: Vec::new(),
        previous_event_hash: previous,
        content_hash: "0".repeat(64),
        hash_version: 2,
        signature: String::new(),
        created_at: now.clone(),
    };
    event.content_hash = event_content_hash(&event);
    validate_event_fields(&event).map_err(to_sql_error)?;

    conn.execute(
        "INSERT INTO events (event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, previous_event_hash, content_hash, hash_version, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            event.event_id,
            event.soul_id,
            event.device_id,
            event.actor,
            event.hlc,
            event.operation,
            event.entity_type,
            event.entity_id,
            event.payload,
            event.previous_event_hash,
            event.content_hash,
            event.hash_version,
            event.created_at
        ],
    )?;

    conn.execute(
        "UPDATE souls SET head_event_hash = ?1 WHERE soul_id = ?2",
        params![event.content_hash, ev.soul_id],
    )?;

    Ok(event.content_hash)
}

pub fn update_entity(
    conn: &Connection,
    soul_id: &str,
    entity_id: &str,
    status: &str,
    data: Option<&str>,
    device_id: &str,
) -> Result<EntityRow, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    validate_status_value(status)?;

    let existing = get_entity(&tx, entity_id)
        .map_err(|e| e.to_string())?
        .ok_or("Entity not found.".to_string())?;
    if existing.soul_id != soul_id {
        // Изоляция между SOUL, §5.9: чужую сущность нельзя редактировать.
        // Тот же ответ, что и при отсутствии сущности, — без раскрытия наличия.
        return Err("Entity not found.".to_string());
    }

    let from = existing.status.as_str();
    let is_edit = from == "candidate" && status == "candidate";

    let allowed = match from {
        "candidate" => matches!(status, "candidate" | "active" | "rejected"),
        "active" => status == "candidate",
        "rejected" => status == "candidate",
        _ => false,
    };
    if !allowed {
        return Err(format!(
            "Status transition {from} -> {status} is not allowed. Reject and undo happen through the candidate state."
        ));
    }

    let next_data = match data {
        Some(d) => {
            if !is_edit {
                return Err(
                    "Entity data can only be changed while editing a candidate.".to_string()
                );
            }
            validate_entity_data_json(d)?;
            d.to_string()
        }
        None => existing.data.clone(),
    };

    if is_edit && data.is_none() {
        return Err("Entity is already a candidate; provide new data to edit it.".to_string());
    }

    if data.is_some() && next_data == existing.data {
        return Err("Entity data is unchanged.".to_string());
    }

    let now = Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE entities SET status = ?1, data = ?2, updated_at = ?3 WHERE id = ?4",
        params![status, next_data, now, entity_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE soul_state SET preview_confirmed = 0 WHERE soul_id = ?1 AND activated = 0",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    let operation = match (from, status) {
        ("candidate", "candidate") => "entity.updated",
        ("candidate", "active") => "entity.activated",
        ("candidate", "rejected") => "entity.rejected",
        (_, "candidate") => "candidate.reopened",
        _ => "entity.updated",
    };

    let payload = serde_json::json!({
        "entityId": entity_id,
        "from": from,
        "status": status,
        "data": next_data
    });
    append_event(
        &tx,
        &NewEvent {
            soul_id: &existing.soul_id,
            device_id,
            actor: "user",
            operation,
            entity_type: &existing.entity_type,
            entity_id,
            payload: &payload,
        },
    )
    .map_err(|e| e.to_string())?;

    bump_state_revision(&tx).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    get_entity(conn, entity_id)
        .map_err(|e| e.to_string())?
        .ok_or("Entity disappeared after update.".to_string())
}

pub fn create_soul(
    conn: &Connection,
    display_name: &str,
    device_id: &str,
) -> SqlResult<SoulManifest> {
    validate_soul_manifest_fields("pending", display_name, device_id).map_err(to_sql_error)?;
    let tx = conn.unchecked_transaction()?;
    let soul_id = format!("soul_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();

    let genesis_payload = serde_json::json!({
        "displayName": display_name,
        "deviceId": device_id
    });

    tx.execute(
        "INSERT INTO souls (soul_id, display_name, created_at, device_id) VALUES (?1, ?2, ?3, ?4)",
        params![soul_id, display_name, now, device_id],
    )?;
    if get_meta(&tx, META_ACTIVE_SOUL_ID)?.is_none() {
        set_meta(&tx, META_ACTIVE_SOUL_ID, &soul_id)?;
    }

    let head = append_event(
        &tx,
        &NewEvent {
            soul_id: &soul_id,
            device_id,
            actor: "user",
            operation: "soul.created",
            entity_type: "fact",
            entity_id: &soul_id,
            payload: &genesis_payload,
        },
    )?;

    tx.commit()?;

    Ok(SoulManifest {
        soul_id,
        display_name: display_name.to_string(),
        format_version: "0.1.0".to_string(),
        schema_version: "0.1.0".to_string(),
        created_at: now,
        head_event_hash: Some(head),
        entity_count: 0,
        device_id: device_id.to_string(),
    })
}

/// Детерминированный ключ дедупликации для ответов калибровки:
/// хэш от (questionId + канонический value). Повторная компиляция одних и тех же
/// ответов возвращает ту же сущность без дубликатов и без лишних событий.
/// Для legacy-данных без questionId/value используется claim как основа ключа.
fn dedup_key_for(data: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    let obj = value.as_object()?;
    if obj.get("source").and_then(|s| s.as_str()) != Some("calibration") {
        return None;
    }
    let question_id = obj.get("questionId").and_then(|q| q.as_str()).unwrap_or("");
    let base = match obj.get("value") {
        Some(v) => {
            if question_id.is_empty() {
                return None;
            }
            serde_json::to_string(v).ok()?
        }
        None => {
            let claim = obj.get("claim").and_then(|c| c.as_str())?;
            serde_json::to_string(claim).ok()?
        }
    };
    Some(compute_hash(&format!("{question_id}\x1f{base}")))
}

fn find_by_dedup_key(
    conn: &Connection,
    soul_id: &str,
    dedup_key: &str,
) -> SqlResult<Option<EntityRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, soul_id, entity_type, status, data, created_at, updated_at
         FROM entities WHERE soul_id = ?1 AND dedup_key = ?2",
    )?;
    let mut rows = stmt.query_map(params![soul_id, dedup_key], |row| {
        Ok(EntityRow {
            id: row.get(0)?,
            soul_id: row.get(1)?,
            entity_type: row.get(2)?,
            status: row.get(3)?,
            data: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn add_entity(
    conn: &Connection,
    soul_id: &str,
    entity_type: &str,
    status: &str,
    data: &str,
    device_id: &str,
) -> Result<EntityRow, String> {
    validate_entity_type_value(entity_type)?;
    validate_status_value(status)?;
    validate_entity_data_json(data)?;
    if get_soul(conn, soul_id)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("SOUL not found.".to_string());
    }
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE soul_id = ?1",
            params![soul_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count >= MAX_ENTITIES_PER_SOUL as i64 {
        return Err(format!(
            "Too many entities for this SOUL (limit {MAX_ENTITIES_PER_SOUL})."
        ));
    }

    if let Some(dedup_key) = dedup_key_for(data) {
        if let Some(existing) =
            find_by_dedup_key(conn, soul_id, &dedup_key).map_err(|e| e.to_string())?
        {
            return Ok(existing);
        }
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let id = format!("ent_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let dedup_key = dedup_key_for(data);

    tx.execute(
        "INSERT INTO entities (id, soul_id, entity_type, status, data, dedup_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, soul_id, entity_type, status, data, dedup_key, now.clone(), now.clone()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE soul_state SET preview_confirmed = 0 WHERE soul_id = ?1 AND activated = 0",
        params![soul_id],
    )
    .map_err(|e| e.to_string())?;

    let operation = match status {
        "candidate" => "candidate.proposed",
        "active" => "entity.activated",
        _ => "entity.updated",
    };

    let payload = serde_json::json!({ "entityId": id, "data": data });
    let head = append_event(
        &tx,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation,
            entity_type,
            entity_id: &id,
            payload: &payload,
        },
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE souls SET entity_count = entity_count + 1, head_event_hash = ?1 WHERE soul_id = ?2",
        params![head, soul_id],
    )
    .map_err(|e| e.to_string())?;

    bump_state_revision(&tx).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(EntityRow {
        id,
        soul_id: soul_id.to_string(),
        entity_type: entity_type.to_string(),
        status: status.to_string(),
        data: data.to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

#[allow(dead_code)]
pub fn list_events(conn: &Connection, soul_id: &str) -> SqlResult<Vec<SoulEvent>> {
    let mut stmt = conn.prepare(
        "SELECT event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, provenance_ids, previous_event_hash, content_hash, hash_version, signature, created_at
         FROM events WHERE soul_id = ?1 ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![soul_id], |row| {
        let provenance_str: String = row.get(9)?;
        Ok(SoulEvent {
            event_id: row.get(0)?,
            soul_id: row.get(1)?,
            device_id: row.get(2)?,
            actor: row.get(3)?,
            hlc: row.get(4)?,
            operation: row.get(5)?,
            entity_type: row.get(6)?,
            entity_id: row.get(7)?,
            payload: row.get(8)?,
            provenance_ids: serde_json::from_str(&provenance_str).unwrap_or_default(),
            previous_event_hash: row.get(10)?,
            content_hash: row.get(11)?,
            hash_version: row.get(12)?,
            signature: row.get(13)?,
            created_at: row.get(14)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_entities(conn: &Connection, soul_id: &str) -> SqlResult<Vec<EntityRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, soul_id, entity_type, status, data, created_at, updated_at
         FROM entities WHERE soul_id = ?1 ORDER BY created_at DESC LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![soul_id, MAX_LIST_ENTITIES as i64], |row| {
        Ok(EntityRow {
            id: row.get(0)?,
            soul_id: row.get(1)?,
            entity_type: row.get(2)?,
            status: row.get(3)?,
            data: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_soul(conn: &Connection, soul_id: &str) -> SqlResult<Option<SoulManifest>> {
    let mut stmt = conn.prepare(
        "SELECT soul_id, display_name, format_version, schema_version, created_at, head_event_hash, entity_count, device_id
         FROM souls WHERE soul_id = ?1",
    )?;

    let mut rows = stmt.query_map(params![soul_id], |row| {
        Ok(SoulManifest {
            soul_id: row.get(0)?,
            display_name: row.get(1)?,
            format_version: row.get(2)?,
            schema_version: row.get(3)?,
            created_at: row.get(4)?,
            head_event_hash: row.get(5)?,
            entity_count: row.get(6)?,
            device_id: row.get(7)?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn list_souls(conn: &Connection) -> SqlResult<Vec<SoulManifest>> {
    let mut stmt = conn.prepare(
        "SELECT soul_id, display_name, format_version, schema_version, created_at, head_event_hash, entity_count, device_id
         FROM souls ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SoulManifest {
            soul_id: row.get(0)?,
            display_name: row.get(1)?,
            format_version: row.get(2)?,
            schema_version: row.get(3)?,
            created_at: row.get(4)?,
            head_event_hash: row.get(5)?,
            entity_count: row.get(6)?,
            device_id: row.get(7)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TestEnv {
        dir: std::path::PathBuf,
        conn: Connection,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-db-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            TestEnv { dir, conn }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn seed(env: &TestEnv) -> (String, String) {
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let ent = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            r#"{"claim":"Prefer concise","source":"calibration"}"#,
            "device_t",
        )
        .unwrap();
        (soul.soul_id, ent.id)
    }

    fn operations(env: &TestEnv, soul_id: &str) -> Vec<String> {
        list_events(&env.conn, soul_id)
            .unwrap()
            .into_iter()
            .map(|e| e.operation)
            .collect()
    }

    #[test]
    fn confirm_activates_and_appends_chained_event() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        let head_before = get_soul(&env.conn, &soul_id)
            .unwrap()
            .unwrap()
            .head_event_hash;

        let row = update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(row.status, "active");
        assert_eq!(
            row.data,
            r#"{"claim":"Prefer concise","source":"calibration"}"#
        );

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.activated".to_string()));
        let soul = get_soul(&env.conn, &soul_id).unwrap().unwrap();
        assert_ne!(soul.head_event_hash, head_before);
        assert_eq!(
            soul.head_event_hash.as_deref(),
            list_events(&env.conn, &soul_id)
                .unwrap()
                .last()
                .map(|e| e.content_hash.as_str())
        );
    }

    #[test]
    fn reject_then_undo_then_confirm_works() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        update_entity(&env.conn, &soul_id, &ent_id, "rejected", None, "device_t").unwrap();
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "rejected"
        );

        update_entity(&env.conn, &soul_id, &ent_id, "candidate", None, "device_t").unwrap();
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "candidate"
        );

        update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "active"
        );

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.rejected".to_string()));
        assert!(ops.contains(&"candidate.reopened".to_string()));
        assert!(ops.contains(&"entity.activated".to_string()));
    }

    #[test]
    fn rejected_cannot_be_activated_directly() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        update_entity(&env.conn, &soul_id, &ent_id, "rejected", None, "device_t").unwrap();

        let err =
            update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap_err();
        assert!(err.contains("not allowed"), "unexpected error: {err}");
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "rejected"
        );
    }

    #[test]
    fn active_cannot_be_rejected_directly() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();

        let err =
            update_entity(&env.conn, &soul_id, &ent_id, "rejected", None, "device_t").unwrap_err();
        assert!(err.contains("not allowed"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_status_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let err =
            update_entity(&env.conn, &soul_id, &ent_id, "disputed", None, "device_t").unwrap_err();
        assert!(
            err.contains("Unknown entity status"),
            "unexpected error: {err}"
        );
        let err =
            update_entity(&env.conn, &soul_id, &ent_id, "deleted", None, "device_t").unwrap_err();
        assert!(
            err.contains("Unknown entity status"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn confirm_preserves_existing_data() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let row = update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(
            row.data,
            r#"{"claim":"Prefer concise","source":"calibration"}"#
        );
    }

    #[test]
    fn edit_candidate_updates_claim_and_creates_event() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let new_data =
            r#"{"claim":"Prefer very concise answers","source":"calibration","confidence":0.9}"#;
        let row = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "candidate",
            Some(new_data),
            "device_t",
        )
        .unwrap();
        assert_eq!(row.status, "candidate");
        assert_eq!(row.data, new_data);

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.updated".to_string()));
    }

    #[test]
    fn edit_active_entity_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();

        let err = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "active",
            Some(r#"{"claim":"changed"}"#),
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("candidate"), "unexpected error: {err}");
    }

    #[test]
    fn invalid_or_oversized_data_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let err = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "candidate",
            Some("not json"),
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("valid JSON"), "unexpected error: {err}");

        let err = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "candidate",
            Some(r#"[1,2,3]"#),
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("JSON object"), "unexpected error: {err}");

        let long_claim = format!(r#"{{"claim":"{}"}}"#, "x".repeat(MAX_CLAIM_CHARS + 1));
        let err = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "candidate",
            Some(&long_claim),
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("too long"), "unexpected error: {err}");
    }

    #[test]
    fn unchanged_data_edit_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let err = update_entity(
            &env.conn,
            &soul_id,
            &ent_id,
            "candidate",
            Some(r#"{"claim":"Prefer concise","source":"calibration"}"#),
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("unchanged"), "unexpected error: {err}");
    }

    #[test]
    fn missing_entity_is_rejected() {
        let env = TestEnv::new();
        let err = update_entity(
            &env.conn,
            "soul_ghost",
            "ent_missing",
            "active",
            None,
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn foreign_soul_cannot_update_entity() {
        let env = TestEnv::new();
        let (soul_a, ent_id) = seed(&env);
        let soul_b = create_soul(&env.conn, "Другая", "device_t")
            .unwrap()
            .soul_id;

        // Чужая душа не может ни активировать, ни редактировать сущность soul_a.
        let err =
            update_entity(&env.conn, &soul_b, &ent_id, "active", None, "device_t").unwrap_err();
        assert_eq!(err, "Entity not found.", "unexpected error: {err}");
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "candidate",
            "entity must stay untouched"
        );

        let err = update_entity(
            &env.conn,
            &soul_b,
            &ent_id,
            "candidate",
            Some(r#"{"claim":"evil"}"#),
            "device_t",
        )
        .unwrap_err();
        assert_eq!(err, "Entity not found.", "unexpected error: {err}");
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().data,
            r#"{"claim":"Prefer concise","source":"calibration"}"#,
            "data must stay untouched"
        );

        // Своя душа по-прежнему может редактировать.
        update_entity(&env.conn, &soul_a, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "active"
        );
    }

    #[test]
    fn noop_candidate_edit_without_data_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let err =
            update_entity(&env.conn, &soul_id, &ent_id, "candidate", None, "device_t").unwrap_err();
        assert!(
            err.contains("already a candidate"),
            "unexpected error: {err}"
        );
        assert_eq!(
            get_entity(&env.conn, &ent_id).unwrap().unwrap().status,
            "candidate"
        );
        assert_eq!(operations(&env, &soul_id).len(), 2);
    }

    #[test]
    fn add_entity_rejects_invalid_status_and_data() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        let err = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "disputed",
            r#"{}"#,
            "device_t",
        )
        .unwrap_err();
        assert!(
            err.contains("Unknown entity status"),
            "unexpected error: {err}"
        );

        let err = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            "not json",
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("valid JSON"), "unexpected error: {err}");

        let err = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            r#"[1,2]"#,
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("JSON object"), "unexpected error: {err}");
    }

    #[test]
    fn event_chain_is_linked_via_previous_hashes() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        update_entity(&env.conn, &soul_id, &ent_id, "active", None, "device_t").unwrap();
        update_entity(&env.conn, &soul_id, &ent_id, "candidate", None, "device_t").unwrap();

        let events = list_events(&env.conn, &soul_id).unwrap();
        assert!(events.len() >= 4);
        let mut prev: Option<String> = None;
        for ev in &events {
            assert_eq!(ev.previous_event_hash, prev);
            prev = Some(ev.content_hash.clone());
        }
    }

    fn calibration_data(question_id: &str, value: &str) -> String {
        format!(
            r#"{{"claim":"Q — {value}","source":"calibration","questionId":"{question_id}","value":"{value}","confidence":0.9}}"#
        )
    }

    #[test]
    fn add_entity_rejects_non_p0_entity_types() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        let err = add_entity(
            &env.conn,
            &soul.soul_id,
            "personality",
            "candidate",
            r#"{"claim":"x"}"#,
            "device_t",
        )
        .unwrap_err();
        assert!(
            err.contains("Unknown entity type"),
            "unexpected error: {err}"
        );
        let err = add_entity(
            &env.conn,
            &soul.soul_id,
            "relationship",
            "candidate",
            r#"{"claim":"x"}"#,
            "device_t",
        )
        .unwrap_err();
        assert!(
            err.contains("Unknown entity type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn add_entity_is_idempotent_for_same_calibration_answer() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let data = calibration_data("pref_1", "Concise");

        let first = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &data,
            "device_t",
        )
        .unwrap();
        let second = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &data,
            "device_t",
        )
        .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(list_entities(&env.conn, &soul.soul_id).unwrap().len(), 1);
        assert_eq!(list_events(&env.conn, &soul.soul_id).unwrap().len(), 2);
        let soul_after = get_soul(&env.conn, &soul.soul_id).unwrap().unwrap();
        assert_eq!(soul_after.entity_count, 1);
    }

    #[test]
    fn add_entity_dedup_distinguishes_answer_values() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        let a = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &calibration_data("pref_1", "Concise"),
            "device_t",
        )
        .unwrap();
        let b = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &calibration_data("pref_1", "Detailed"),
            "device_t",
        )
        .unwrap();

        assert_ne!(a.id, b.id);
        assert_eq!(list_entities(&env.conn, &soul.soul_id).unwrap().len(), 2);
    }

    #[test]
    fn add_entity_dedup_falls_back_to_claim_for_legacy_data() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let data = r#"{"claim":"Prefer concise answers","source":"calibration"}"#;

        let first = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            data,
            "device_t",
        )
        .unwrap();
        let second = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            data,
            "device_t",
        )
        .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(list_entities(&env.conn, &soul.soul_id).unwrap().len(), 1);

        let different = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            r#"{"claim":"Prefer verbose answers","source":"calibration"}"#,
            "device_t",
        )
        .unwrap();
        assert_ne!(first.id, different.id);
        assert_eq!(list_entities(&env.conn, &soul.soul_id).unwrap().len(), 2);
    }

    #[test]
    fn add_entity_without_calibration_source_is_not_deduped() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let data = r#"{"claim":"x","questionId":"pref_1","value":"Concise"}"#;

        let first = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            data,
            "device_t",
        )
        .unwrap();
        let second = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            data,
            "device_t",
        )
        .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(list_entities(&env.conn, &soul.soul_id).unwrap().len(), 2);
    }

    #[test]
    fn migration_adds_new_columns_to_old_schema_and_keeps_data() {
        let dir = std::env::temp_dir().join(format!("soul-db-migrate-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let old = rusqlite::Connection::open(dir.join("soul.db")).unwrap();
        old.execute_batch(
            r#"CREATE TABLE souls (
                soul_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                format_version TEXT NOT NULL DEFAULT '0.1.0',
                schema_version TEXT NOT NULL DEFAULT '0.1.0',
                created_at TEXT NOT NULL,
                head_event_hash TEXT,
                entity_count INTEGER NOT NULL DEFAULT 0,
                device_id TEXT NOT NULL
            );
            CREATE TABLE events (
                event_id TEXT PRIMARY KEY,
                soul_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                hlc TEXT NOT NULL,
                operation TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}',
                provenance_ids TEXT NOT NULL DEFAULT '[]',
                previous_event_hash TEXT,
                content_hash TEXT NOT NULL,
                signature TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            );
            CREATE TABLE entities (
                id TEXT PRIMARY KEY,
                soul_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                data TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE soul_state (
                soul_id TEXT PRIMARY KEY,
                activated INTEGER NOT NULL DEFAULT 0,
                calibration_step INTEGER NOT NULL DEFAULT 0,
                calibration_answers TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO souls (soul_id, display_name, created_at, device_id)
                VALUES ('soul_old', 'Старый', '2026-07-01T00:00:00Z', 'device_old');
            INSERT INTO entities (id, soul_id, entity_type, status, data, created_at, updated_at)
                VALUES ('ent_old', 'soul_old', 'preference', 'active', '{"claim":"old"}', '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z');
            INSERT INTO soul_state (soul_id, activated)
                VALUES ('soul_old', 1);
            INSERT INTO meta (key, value) VALUES
                ('custom_meta', 'kept'),
                ('imported_content_hash', 'obsolete'),
                ('imported_soul_id', 'soul_old'),
                ('imported_state_revision', '7');"#,
        )
        .unwrap();
        crate::eval::init_evaluations(&old).unwrap();
        crate::policy::init_policies(&old).unwrap();
        crate::gateway::init_gateway(&old).unwrap();
        old.execute_batch(
            r#"INSERT INTO evaluations (
                    id, soul_id, scenario_id, scenario_text, domain, soul_variant,
                    soul_answer, baseline_answer, created_at
                ) VALUES (
                    'eval_old', 'soul_old', 'scenario_old', 'Scenario', 'writing',
                    'a', 'SOUL answer', 'Baseline answer', '2026-07-01T00:00:00Z'
                );
                INSERT INTO policies (id, priority, enabled, rule_json, created_at, updated_at)
                VALUES (
                    'policy_old', 500, 1, '{"id":"policy_old","effect":"allow"}',
                    '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z'
                );
                INSERT INTO capabilities (
                    id, action_id, kind, payload_hash, nonce, action_json, expires_at,
                    created_at, connector_id, account_id, environment, decision_effect
                ) VALUES (
                    'cap_old', 'action_old', 'message.send', 'hash', 'nonce_old', '{}',
                    '2026-07-02T00:00:00Z', '2026-07-01T00:00:00Z',
                    'connector_old', 'account_old', 'sandbox', 'allow'
                );
                INSERT INTO gateway_receipts (
                    id, capability_id, action_id, kind, status, decision_effect,
                    connector_executed, created_at
                ) VALUES (
                    'receipt_old', 'cap_old', 'action_old', 'message.send', 'pending',
                    'allow', 0, '2026-07-01T00:00:00Z'
                );
                INSERT OR REPLACE INTO gateway_connectors
                    (connector_id, account_id, environment)
                VALUES ('connector_old', 'account_old', 'sandbox');
                INSERT OR REPLACE INTO gateway_meta (key, value)
                VALUES ('custom_gateway_meta', 'kept');"#,
        )
        .unwrap();
        init_fts(&old).unwrap();
        assert!(old
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'entity_fts_data')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
        drop(old);

        let conn = init_db(&dir).unwrap();

        let cols = conn
            .prepare("PRAGMA table_info(entities)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Vec<_>>();
        assert!(cols.iter().any(|c| c.as_deref() == Ok("dedup_key")));

        let state_cols = conn
            .prepare("PRAGMA table_info(soul_state)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Vec<_>>();
        assert!(state_cols
            .iter()
            .any(|c| c.as_deref() == Ok("preview_confirmed")));

        let soul = get_soul(&conn, "soul_old").unwrap().unwrap();
        assert_eq!(soul.display_name, "Старый");
        assert_eq!(list_entities(&conn, "soul_old").unwrap().len(), 1);
        assert!(is_soul_activated(&conn, "soul_old").unwrap());
        assert!(!get_soul_state(&conn, "soul_old").unwrap().3);
        assert_eq!(
            get_meta(&conn, "custom_meta").unwrap().as_deref(),
            Some("kept")
        );
        assert!(get_meta(&conn, "imported_content_hash").unwrap().is_none());
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM evaluations WHERE id = 'eval_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM policies WHERE id = 'policy_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM capabilities WHERE id = 'cap_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM gateway_receipts WHERE id = 'receipt_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM gateway_connectors WHERE connector_id = 'connector_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT value FROM gateway_meta WHERE key = 'custom_gateway_meta'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "kept"
        );
        let (fts_rows, _) = search_entities(&conn, "soul_old", "old", 10).unwrap();
        assert_eq!(fts_rows.len(), 1);

        init_db(&dir).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_recovers_completed_encrypted_migration_swap() {
        let dir = std::env::temp_dir().join(format!("soul-db-recovery-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = init_db(&dir).unwrap();
        let soul = create_soul(&conn, "Recovery", "device_r").unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);

        std::fs::rename(dir.join("soul.db"), dir.join("soul.new")).unwrap();
        let recovered = init_db(&dir).unwrap();
        assert!(get_soul(&recovered, &soul.soul_id).unwrap().is_some());
        assert!(dir.join("soul.db").exists());
        assert!(!dir.join("soul.new").exists());
        drop(recovered);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn confirm_preview_is_idempotent_and_appends_one_event() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t").unwrap();
        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t").unwrap();

        let ops: Vec<String> = list_events(&env.conn, &soul.soul_id)
            .unwrap()
            .into_iter()
            .map(|e| e.operation)
            .collect();
        assert_eq!(
            ops.iter()
                .filter(|o| o.as_str() == "soul.preview_confirmed")
                .count(),
            1
        );
        assert!(get_soul_state(&env.conn, &soul.soul_id).unwrap().3);
    }

    #[test]
    fn activate_requires_preview_confirmation() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        let err = activate_soul(&env.conn, &soul.soul_id, "device_t").unwrap_err();
        assert!(
            err.contains("preview is confirmed"),
            "unexpected error: {err}"
        );
        assert!(!is_soul_activated(&env.conn, &soul.soul_id).unwrap());
    }

    #[test]
    fn activate_after_preview_confirmation_appends_event() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();

        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t").unwrap();
        activate_soul(&env.conn, &soul.soul_id, "device_t").unwrap();

        assert!(is_soul_activated(&env.conn, &soul.soul_id).unwrap());
        let ops: Vec<String> = list_events(&env.conn, &soul.soul_id)
            .unwrap()
            .into_iter()
            .map(|e| e.operation)
            .collect();
        assert!(ops.contains(&"soul.activated".to_string()));
    }

    fn preview_seed(env: &TestEnv) -> (String, Vec<String>) {
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let mut ids = Vec::new();
        for (question_id, value) in [("pref_1", "Concise"), ("goal_1", "Build a product")] {
            let e = add_entity(
                &env.conn,
                &soul.soul_id,
                if question_id.starts_with("pref_") {
                    "preference"
                } else {
                    "goal"
                },
                "candidate",
                &calibration_data(question_id, value),
                "device_t",
            )
            .unwrap();
            ids.push(e.id);
        }
        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t").unwrap();
        (soul.soul_id, ids)
    }

    #[test]
    fn activate_preview_requires_preview_confirmation() {
        let env = TestEnv::new();
        let (soul_id, ids) = {
            let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
            let e = add_entity(
                &env.conn,
                &soul.soul_id,
                "preference",
                "candidate",
                &calibration_data("pref_1", "Concise"),
                "device_t",
            )
            .unwrap();
            (soul.soul_id, vec![e.id])
        };

        let err = activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap_err();
        assert!(
            err.contains("preview is confirmed"),
            "unexpected error: {err}"
        );
        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());
        assert_eq!(
            get_entity(&env.conn, &ids[0]).unwrap().unwrap().status,
            "candidate"
        );
    }

    #[test]
    fn reset_preview_is_idempotent_and_writes_revoked_event_once() {
        let env = TestEnv::new();
        let (soul_id, _) = preview_seed(&env);
        assert!(get_soul_state(&env.conn, &soul_id).unwrap().3);

        reset_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        assert!(!get_soul_state(&env.conn, &soul_id).unwrap().3);
        reset_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        assert!(!get_soul_state(&env.conn, &soul_id).unwrap().3);

        let ops: Vec<String> = list_events(&env.conn, &soul_id)
            .unwrap()
            .into_iter()
            .map(|e| e.operation)
            .collect();
        assert_eq!(
            ops.iter()
                .filter(|o| o.as_str() == "soul.preview_revoked")
                .count(),
            1
        );
        assert!(ops.contains(&"soul.preview_confirmed".to_string()));
        assert!(!ops.contains(&"soul.activated".to_string()));
    }

    #[test]
    fn reset_preview_blocks_activation_until_reconfirm() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);

        reset_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        let err = activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap_err();
        assert!(
            err.contains("preview is confirmed"),
            "unexpected error: {err}"
        );
        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());

        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap();
        assert!(is_soul_activated(&env.conn, &soul_id).unwrap());
    }

    #[test]
    fn reset_preview_after_activation_is_rejected() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap();

        let err = reset_soul_preview(&env.conn, &soul_id, "device_t").unwrap_err();
        assert!(err.contains("after activation"), "unexpected error: {err}");
        assert!(get_soul_state(&env.conn, &soul_id).unwrap().3);
    }

    #[test]
    fn activate_preview_activates_eligible_candidates() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);

        activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap();

        assert!(is_soul_activated(&env.conn, &soul_id).unwrap());
        assert!(get_soul_state(&env.conn, &soul_id).unwrap().2);
        for id in &ids {
            assert_eq!(get_entity(&env.conn, id).unwrap().unwrap().status, "active");
        }
        let ops: Vec<String> = list_events(&env.conn, &soul_id)
            .unwrap()
            .into_iter()
            .map(|e| e.operation)
            .collect();
        assert_eq!(
            ops.iter()
                .filter(|o| o.as_str() == "entity.activated")
                .count(),
            2
        );
        assert!(ops.contains(&"soul.activated".to_string()));
    }

    #[test]
    fn activate_preview_rejects_boundary_and_activates_nothing() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        let boundary = add_entity(
            &env.conn,
            &soul_id,
            "boundary",
            "candidate",
            r#"{"claim":"Never decide finances","source":"calibration","questionId":"bound_1","value":"Financial decisions"}"#,
            "device_t",
        )
        .unwrap();

        let mut with_boundary = ids.clone();
        with_boundary.push(boundary.id.clone());
        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        let err = activate_preview(&env.conn, &soul_id, &with_boundary, "device_t").unwrap_err();
        assert!(
            err.contains("cannot be activated by preview confirmation"),
            "unexpected error: {err}"
        );

        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());
        for id in &ids {
            assert_eq!(
                get_entity(&env.conn, id).unwrap().unwrap().status,
                "candidate"
            );
        }
    }

    #[test]
    fn activate_preview_rejects_sensitive_and_risk_flagged() {
        let env = TestEnv::new();
        let (soul_id, mut ids) = preview_seed(&env);

        let sensitive = add_entity(
            &env.conn,
            &soul_id,
            "fact",
            "candidate",
            r#"{"claim":"My password is x","source":"calibration","questionId":"text_1","value":"My password is x","sensitivity":"sensitive"}"#,
            "device_t",
        )
        .unwrap();
        let risk = add_entity(
            &env.conn,
            &soul_id,
            "preference",
            "candidate",
            r#"{"claim":"risky","source":"calibration","questionId":"pref_9","value":"Speed","risk":true}"#,
            "device_t",
        )
        .unwrap();

        ids.push(sensitive.id.clone());
        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        let err = activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap_err();
        assert!(
            err.contains("cannot be activated by preview confirmation"),
            "unexpected error: {err}"
        );

        let mut with_risk = vec![risk.id.clone()];
        with_risk.push(ids[0].clone());
        let err = activate_preview(&env.conn, &soul_id, &with_risk, "device_t").unwrap_err();
        assert!(
            err.contains("cannot be activated by preview confirmation"),
            "unexpected error: {err}"
        );

        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());
    }

    #[test]
    fn activate_preview_rejects_foreign_or_missing_entity() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        let other = create_soul(&env.conn, "Другой", "device_t").unwrap();
        let foreign = add_entity(
            &env.conn,
            &other.soul_id,
            "preference",
            "candidate",
            &calibration_data("pref_1", "Concise"),
            "device_t",
        )
        .unwrap();

        let mut with_foreign = ids.clone();
        with_foreign.push(foreign.id.clone());
        let err = activate_preview(&env.conn, &soul_id, &with_foreign, "device_t").unwrap_err();
        assert!(err.contains("does not belong"), "unexpected error: {err}");

        let err = activate_preview(
            &env.conn,
            &soul_id,
            &["ent_missing".to_string()],
            "device_t",
        )
        .unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");

        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());
    }

    #[test]
    fn activate_preview_rejects_rejected_entity() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        let rejected = add_entity(
            &env.conn,
            &soul_id,
            "preference",
            "candidate",
            &calibration_data("pref_2", "Bullet points"),
            "device_t",
        )
        .unwrap();
        update_entity(
            &env.conn,
            &soul_id,
            &rejected.id,
            "rejected",
            None,
            "device_t",
        )
        .unwrap();

        let mut with_rejected = ids.clone();
        with_rejected.push(rejected.id);
        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        let err = activate_preview(&env.conn, &soul_id, &with_rejected, "device_t").unwrap_err();
        assert!(
            err.contains("cannot be activated by preview confirmation"),
            "unexpected error: {err}"
        );
        assert!(!is_soul_activated(&env.conn, &soul_id).unwrap());
    }

    #[test]
    fn activate_preview_skips_already_active_entities() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        update_entity(&env.conn, &soul_id, &ids[0], "active", None, "device_t").unwrap();
        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();

        activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap();

        assert!(is_soul_activated(&env.conn, &soul_id).unwrap());
        assert_eq!(
            get_entity(&env.conn, &ids[0]).unwrap().unwrap().status,
            "active"
        );
        assert_eq!(
            get_entity(&env.conn, &ids[1]).unwrap().unwrap().status,
            "active"
        );
    }

    #[test]
    fn activate_preview_fails_when_soul_already_activated() {
        let env = TestEnv::new();
        let (soul_id, ids) = preview_seed(&env);
        confirm_soul_preview(&env.conn, &soul_id, "device_t").unwrap();
        activate_soul(&env.conn, &soul_id, "device_t").unwrap();

        let err = activate_preview(&env.conn, &soul_id, &ids, "device_t").unwrap_err();
        assert!(err.contains("already activated"), "unexpected error: {err}");
    }

    #[test]
    fn activate_preview_with_empty_list_activates_soul() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t").unwrap();

        activate_preview(&env.conn, &soul.soul_id, &[], "device_t").unwrap();

        assert!(is_soul_activated(&env.conn, &soul.soul_id).unwrap());
        assert!(get_soul_state(&env.conn, &soul.soul_id).unwrap().2);
    }

    fn fts_data(question_id: &str, value: &str, claim: &str) -> String {
        format!(
            r#"{{"claim":"{claim}","evidence":"Q — {value}","source":"calibration","questionId":"{question_id}","value":"{value}","confidence":0.9,"sensitivity":"internal","scope":{{"domains":["preferences"],"projects":[],"people":[],"channels":[]}},"risk":false}}"#
        )
    }

    fn seed_fts_entities(env: &TestEnv, soul_id: &str, status: &str) {
        let items = [
            ("pref_1", "Concise", "Prefer concise technical answers"),
            ("pref_2", "Bullet points", "Prefer bullet points in lists"),
            (
                "goal_1",
                "Build a product",
                "Primary goal is building a product",
            ),
        ];
        for (qid, value, claim) in items {
            add_entity(
                &env.conn,
                soul_id,
                "preference",
                status,
                &fts_data(qid, value, claim),
                "device_t",
            )
            .unwrap();
        }
    }

    #[test]
    fn fts5_is_enabled_in_bundled_sqlite() {
        let env = TestEnv::new();
        let options: String = env
            .conn
            .query_row(
                "SELECT group_concat(compile_options, ' ') FROM pragma_compile_options",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            options.contains("ENABLE_FTS5"),
            "FTS5 must be compiled in: {options}"
        );
    }

    #[test]
    fn fts_finds_entities_by_claim_and_evidence() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        seed_fts_entities(&env, &soul.soul_id, "candidate");

        let (hits, truncated) = search_entities(&env.conn, &soul.soul_id, "concise", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(!truncated);
        assert!(hits[0].data.contains("Concise"));

        let (hits, truncated) = search_entities(&env.conn, &soul.soul_id, "bullet", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(!truncated);
        assert!(hits[0].data.contains("Bullet points"));
    }

    #[test]
    fn fts_ranks_better_matches_first() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &fts_data(
                "pref_1",
                "Concise",
                "Prefer concise concise technical answers",
            ),
            "device_t",
        )
        .unwrap();
        add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &fts_data("pref_9", "Speed", "Concise code review style"),
            "device_t",
        )
        .unwrap();

        let (hits, _truncated) = search_entities(&env.conn, &soul.soul_id, "concise", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(
            hits[0].data.contains("Prefer concise concise"),
            "entity with higher term frequency must rank first"
        );

        // AND-семантика: слово из одного клайма не должно тянуть чужой результат.
        let (hits, _truncated) =
            search_entities(&env.conn, &soul.soul_id, "concise technical", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].data.contains("Prefer concise concise"));
    }

    #[test]
    fn fts_never_leaks_entities_across_souls() {
        let env = TestEnv::new();
        let soul_a = create_soul(&env.conn, "А", "device_t").unwrap();
        let soul_b = create_soul(&env.conn, "Б", "device_t").unwrap();
        seed_fts_entities(&env, &soul_a.soul_id, "candidate");
        seed_fts_entities(&env, &soul_b.soul_id, "candidate");

        // "prefer" есть в двух сущностях каждой души: проверяем, что результат
        // для soul_a не содержит ни одной чужой сущности.
        let (hits, _truncated) = search_entities(&env.conn, &soul_a.soul_id, "prefer", 10).unwrap();
        assert_eq!(hits.len(), 2);
        for hit in &hits {
            assert_eq!(hit.soul_id, soul_a.soul_id, "foreign entity leaked");
        }
    }

    #[test]
    fn fts_stays_in_sync_with_updates_and_deletes() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        let ent = add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            &fts_data("pref_1", "Concise", "Prefer concise technical answers"),
            "device_t",
        )
        .unwrap();

        let new_data = r#"{"claim":"Prefer extremely verbose prose","evidence":"Q — Long","source":"calibration","questionId":"pref_1","value":"Long","confidence":0.9}"#;
        update_entity(
            &env.conn,
            &soul.soul_id,
            &ent.id,
            "candidate",
            Some(new_data),
            "device_t",
        )
        .unwrap();

        assert!(
            search_entities(&env.conn, &soul.soul_id, "verbose", 10)
                .unwrap()
                .0
                .len()
                == 1
        );
        assert!(search_entities(&env.conn, &soul.soul_id, "concise", 10)
            .unwrap()
            .0
            .is_empty());

        env.conn
            .execute("DELETE FROM entities WHERE id = ?1", params![ent.id])
            .unwrap();
        assert!(search_entities(&env.conn, &soul.soul_id, "verbose", 10)
            .unwrap()
            .0
            .is_empty());
    }

    #[test]
    fn fts_syncs_on_raw_insert_like_import() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        env.conn
            .execute(
                "INSERT INTO entities (id, soul_id, entity_type, status, data, created_at, updated_at)
                 VALUES ('ent_imp', ?1, 'fact', 'active', '{\"claim\":\"Imported memory about signal processing\"}', '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z')",
                params![soul.soul_id],
            )
            .unwrap();

        let (hits, _truncated) = search_entities(&env.conn, &soul.soul_id, "signal", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "ent_imp");
    }

    #[test]
    fn fts_handles_garbage_queries_without_error() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        seed_fts_entities(&env, &soul.soul_id, "candidate");

        for garbage in ["", "   ", "!!!", "\"\"", "()", "AND", "-", "***", "? query"] {
            let (hits, _truncated) =
                search_entities(&env.conn, &soul.soul_id, garbage, 10).unwrap();
            assert_eq!(
                hits.len(),
                0,
                "garbage query {garbage:?} must return nothing"
            );
        }
    }

    #[test]
    fn fts_respects_limit() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        seed_fts_entities(&env, &soul.soul_id, "candidate");
        // Ещё совпадения по "prefer", чтобы результат гарантированно
        // превышал лимит и был помечен как truncated.
        for i in 0..4 {
            add_entity(
                &env.conn,
                &soul.soul_id,
                "preference",
                "candidate",
                &fts_data(&format!("extra_{i}"), "X", "Prefer extra note"),
                "device_t",
            )
            .unwrap();
        }

        let (hits, truncated) = search_entities(&env.conn, &soul.soul_id, "prefer", 2).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(truncated, "more than 2 matches must report truncation");
    }

    #[test]
    fn fts_search_over_thousand_entities_is_fast() {
        let env = TestEnv::new();
        let soul = create_soul(&env.conn, "Тест", "device_t").unwrap();
        for i in 0..1000 {
            add_entity(
                &env.conn,
                &soul.soul_id,
                "fact",
                "active",
                &format!(
                    r#"{{"claim":"Memory item number {i} about topic_{}","source":"calibration","questionId":"text_1","value":"v{i}","confidence":0.5}}"#,
                    i % 20
                ),
                "device_t",
            )
            .unwrap();
        }

        let start = std::time::Instant::now();
        for _ in 0..20 {
            let (hits, _truncated) =
                search_entities(&env.conn, &soul.soul_id, "topic_7", 10).unwrap();
            assert!(!hits.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 1000,
            "20 searches over 1000 entities took {:?}",
            elapsed
        );
    }
}
