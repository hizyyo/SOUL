use rusqlite::{Connection, Result as SqlResult, params};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use chrono::Utc;

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
    pub signature: String,
    pub created_at: String,
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
    let conn = Connection::open(&db_path)?;

    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS souls (
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
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE INDEX IF NOT EXISTS idx_events_soul ON events(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_soul ON entities(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);",
    )?;

    Ok(conn)
}

fn compute_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn create_soul(conn: &Connection, display_name: &str, device_id: &str) -> SqlResult<SoulManifest> {
    let soul_id = format!("soul_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let event_id = format!("evt_{}", Uuid::new_v4());

    let genesis_payload = serde_json::json!({
        "displayName": display_name,
        "deviceId": device_id
    });

    let content_hash = compute_hash(&serde_json::to_string(&genesis_payload).unwrap());

    conn.execute(
        "INSERT INTO souls (soul_id, display_name, created_at, device_id) VALUES (?1, ?2, ?3, ?4)",
        params![soul_id, display_name, now, device_id],
    )?;

    conn.execute(
        "INSERT INTO events (event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, content_hash, created_at)
         VALUES (?1, ?2, ?3, 'user', ?4, 'soul.created', 'fact', ?5, ?6, ?7, ?8)",
        params![event_id, soul_id, device_id, now, soul_id, serde_json::to_string(&genesis_payload).unwrap(), content_hash, now],
    )?;

    conn.execute(
        "UPDATE souls SET head_event_hash = ?1 WHERE soul_id = ?2",
        params![content_hash, soul_id],
    )?;

    Ok(SoulManifest {
        soul_id,
        display_name: display_name.to_string(),
        format_version: "0.1.0".to_string(),
        schema_version: "0.1.0".to_string(),
        created_at: now,
        head_event_hash: Some(content_hash),
        entity_count: 0,
        device_id: device_id.to_string(),
    })
}

pub fn add_entity(
    conn: &Connection,
    soul_id: &str,
    entity_type: &str,
    status: &str,
    data: &str,
    device_id: &str,
) -> SqlResult<EntityRow> {
    let id = format!("ent_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let event_id = format!("evt_{}", Uuid::new_v4());

    conn.execute(
        "INSERT INTO entities (id, soul_id, entity_type, status, data, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, soul_id, entity_type, status, data, now.clone(), now.clone()],
    )?;

    let operation = match status {
        "candidate" => "candidate.proposed",
        "active" => "entity.activated",
        _ => "entity.updated",
    };

    let payload = serde_json::json!({ "entityId": id, "data": data });
    let content_hash = compute_hash(&serde_json::to_string(&payload).unwrap());

    conn.execute(
        "INSERT INTO events (event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, content_hash, created_at)
         VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![event_id, soul_id, device_id, now.clone(), operation, entity_type, id, serde_json::to_string(&payload).unwrap(), content_hash, now.clone()],
    )?;

    conn.execute(
        "UPDATE souls SET entity_count = entity_count + 1, head_event_hash = ?1 WHERE soul_id = ?2",
        params![content_hash, soul_id],
    )?;

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
        "SELECT event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, provenance_ids, previous_event_hash, content_hash, signature, created_at
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
            signature: row.get(12)?,
            created_at: row.get(13)?,
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
         FROM entities WHERE soul_id = ?1 ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map(params![soul_id], |row| {
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
