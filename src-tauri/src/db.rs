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

        CREATE TABLE IF NOT EXISTS soul_state (
            soul_id TEXT PRIMARY KEY,
            activated INTEGER NOT NULL DEFAULT 0,
            calibration_step INTEGER NOT NULL DEFAULT 0,
            calibration_answers TEXT NOT NULL DEFAULT '[]',
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE INDEX IF NOT EXISTS idx_events_soul ON events(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_soul ON entities(soul_id);
        CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);",
    )?;

    Ok(conn)
}

pub fn compute_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn count_soul(conn: &Connection, soul_id: &str) -> SqlResult<(i64, i64)> {
    let entities: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE soul_id = ?1",
        params![soul_id],
        |row| row.get(0),
    )?;
    let events: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE soul_id = ?1",
        params![soul_id],
        |row| row.get(0),
    )?;
    Ok((entities, events))
}

pub fn get_soul_state(conn: &Connection, soul_id: &str) -> SqlResult<(i32, String, bool)> {
    let mut stmt = conn.prepare(
        "SELECT calibration_step, calibration_answers, activated FROM soul_state WHERE soul_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![soul_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?, row.get::<_, i32>(2)? != 0))
    })?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok((0, "[]".to_string(), false)),
    }
}

pub fn wipe_all(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("PRAGMA secure_delete=ON;")?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    conn.execute_batch(
        "DELETE FROM entities;
         DELETE FROM events;
         DELETE FROM soul_state;
         DELETE FROM souls;",
    )?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    conn.execute_batch("VACUUM;")?;
    Ok(())
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
    conn.execute(
        "INSERT INTO soul_state (soul_id, calibration_step, calibration_answers)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(soul_id) DO UPDATE SET
           calibration_step = excluded.calibration_step,
           calibration_answers = excluded.calibration_answers",
        params![soul_id, step, answers],
    )?;
    Ok(())
}

pub fn activate_soul(conn: &Connection, soul_id: &str) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO soul_state (soul_id, activated)
         VALUES (?1, 1)
         ON CONFLICT(soul_id) DO UPDATE SET activated = 1",
        params![soul_id],
    )?;
    Ok(())
}

pub fn is_soul_activated(conn: &Connection, soul_id: &str) -> SqlResult<bool> {
    let mut stmt = conn.prepare(
        "SELECT activated FROM soul_state WHERE soul_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![soul_id], |row| {
        Ok(row.get::<_, i32>(0)? != 0)
    })?;
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

fn validate_status_value(status: &str) -> Result<(), String> {
    if matches!(status, "candidate" | "active" | "rejected") {
        Ok(())
    } else {
        Err(format!("Unknown entity status: {status}"))
    }
}

fn validate_entity_data_json(data: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(data)
        .map_err(|_| "Entity data must be valid JSON.".to_string())?;
    if !value.is_object() {
        return Err("Entity data must be a JSON object.".to_string());
    }
    if let Some(claim) = value.get("claim").and_then(|c| c.as_str()) {
        if claim.chars().count() > MAX_CLAIM_CHARS {
            return Err(format!("Entity claim is too long (limit {MAX_CLAIM_CHARS} characters)."));
        }
    }
    Ok(())
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
    let event_id = format!("evt_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let content_hash = compute_hash(&serde_json::to_string(ev.payload).unwrap());
    let previous = read_soul_head(conn, ev.soul_id)?;

    conn.execute(
        "INSERT INTO events (event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, previous_event_hash, content_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event_id,
            ev.soul_id,
            ev.device_id,
            ev.actor,
            now,
            ev.operation,
            ev.entity_type,
            ev.entity_id,
            serde_json::to_string(ev.payload).unwrap(),
            previous,
            content_hash,
            now
        ],
    )?;

    conn.execute(
        "UPDATE souls SET head_event_hash = ?1 WHERE soul_id = ?2",
        params![content_hash, ev.soul_id],
    )?;

    Ok(content_hash)
}

pub fn update_entity(
    conn: &Connection,
    entity_id: &str,
    status: &str,
    data: Option<&str>,
    device_id: &str,
) -> Result<EntityRow, String> {
    validate_status_value(status)?;

    let existing = get_entity(conn, entity_id)
        .map_err(|e| e.to_string())?
        .ok_or("Entity not found.".to_string())?;

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
                    "Entity data can only be changed while editing a candidate.".to_string(),
                );
            }
            validate_entity_data_json(d)?;
            d.to_string()
        }
        None => existing.data.clone(),
    };

    if data.is_some() && next_data == existing.data {
        return Err("Entity data is unchanged.".to_string());
    }

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE entities SET status = ?1, data = ?2, updated_at = ?3 WHERE id = ?4",
        params![status, next_data, now, entity_id],
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
        conn,
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

    get_entity(conn, entity_id)
        .map_err(|e| e.to_string())?
        .ok_or("Entity disappeared after update.".to_string())
}

pub fn create_soul(conn: &Connection, display_name: &str, device_id: &str) -> SqlResult<SoulManifest> {
    let soul_id = format!("soul_{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();

    let genesis_payload = serde_json::json!({
        "displayName": display_name,
        "deviceId": device_id
    });

    conn.execute(
        "INSERT INTO souls (soul_id, display_name, created_at, device_id) VALUES (?1, ?2, ?3, ?4)",
        params![soul_id, display_name, now, device_id],
    )?;

    let head = append_event(
        conn,
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
    let head = append_event(
        conn,
        &NewEvent {
            soul_id,
            device_id,
            actor: "user",
            operation,
            entity_type,
            entity_id: &id,
            payload: &payload,
        },
    )?;

    conn.execute(
        "UPDATE souls SET entity_count = entity_count + 1, head_event_hash = ?1 WHERE soul_id = ?2",
        params![head, soul_id],
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
        let head_before = get_soul(&env.conn, &soul_id).unwrap().unwrap().head_event_hash;

        let row = update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(row.status, "active");
        assert_eq!(row.data, r#"{"claim":"Prefer concise","source":"calibration"}"#);

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.activated".to_string()));
        let soul = get_soul(&env.conn, &soul_id).unwrap().unwrap();
        assert_ne!(soul.head_event_hash, head_before);
        assert_eq!(
            soul.head_event_hash.as_deref(),
            list_events(&env.conn, &soul_id).unwrap().last().map(|e| e.content_hash.as_str())
        );
    }

    #[test]
    fn reject_then_undo_then_confirm_works() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        update_entity(&env.conn, &ent_id, "rejected", None, "device_t").unwrap();
        assert_eq!(get_entity(&env.conn, &ent_id).unwrap().unwrap().status, "rejected");

        update_entity(&env.conn, &ent_id, "candidate", None, "device_t").unwrap();
        assert_eq!(get_entity(&env.conn, &ent_id).unwrap().unwrap().status, "candidate");

        update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(get_entity(&env.conn, &ent_id).unwrap().unwrap().status, "active");

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.rejected".to_string()));
        assert!(ops.contains(&"candidate.reopened".to_string()));
        assert!(ops.contains(&"entity.activated".to_string()));
    }

    #[test]
    fn rejected_cannot_be_activated_directly() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);
        update_entity(&env.conn, &ent_id, "rejected", None, "device_t").unwrap();

        let err = update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap_err();
        assert!(err.contains("not allowed"), "unexpected error: {err}");
        assert_eq!(get_entity(&env.conn, &ent_id).unwrap().unwrap().status, "rejected");
    }

    #[test]
    fn active_cannot_be_rejected_directly() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);
        update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();

        let err = update_entity(&env.conn, &ent_id, "rejected", None, "device_t").unwrap_err();
        assert!(err.contains("not allowed"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_status_is_rejected() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);

        let err = update_entity(&env.conn, &ent_id, "disputed", None, "device_t").unwrap_err();
        assert!(err.contains("Unknown entity status"), "unexpected error: {err}");
        let err = update_entity(&env.conn, &ent_id, "deleted", None, "device_t").unwrap_err();
        assert!(err.contains("Unknown entity status"), "unexpected error: {err}");
    }

    #[test]
    fn confirm_preserves_existing_data() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);

        let row = update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();
        assert_eq!(row.data, r#"{"claim":"Prefer concise","source":"calibration"}"#);
    }

    #[test]
    fn edit_candidate_updates_claim_and_creates_event() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);

        let new_data = r#"{"claim":"Prefer very concise answers","source":"calibration","confidence":0.9}"#;
        let row = update_entity(&env.conn, &ent_id, "candidate", Some(new_data), "device_t").unwrap();
        assert_eq!(row.status, "candidate");
        assert_eq!(row.data, new_data);

        let ops = operations(&env, &soul_id);
        assert!(ops.contains(&"entity.updated".to_string()));
    }

    #[test]
    fn edit_active_entity_is_rejected() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);
        update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();

        let err = update_entity(
            &env.conn,
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
        let (_, ent_id) = seed(&env);

        let err = update_entity(&env.conn, &ent_id, "candidate", Some("not json"), "device_t").unwrap_err();
        assert!(err.contains("valid JSON"), "unexpected error: {err}");

        let err = update_entity(&env.conn, &ent_id, "candidate", Some(r#"[1,2,3]"#), "device_t").unwrap_err();
        assert!(err.contains("JSON object"), "unexpected error: {err}");

        let long_claim = format!(r#"{{"claim":"{}"}}"#, "x".repeat(MAX_CLAIM_CHARS + 1));
        let err = update_entity(&env.conn, &ent_id, "candidate", Some(&long_claim), "device_t").unwrap_err();
        assert!(err.contains("too long"), "unexpected error: {err}");
    }

    #[test]
    fn unchanged_data_edit_is_rejected() {
        let env = TestEnv::new();
        let (_, ent_id) = seed(&env);

        let err = update_entity(
            &env.conn,
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
        let err = update_entity(&env.conn, "ent_missing", "active", None, "device_t").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn event_chain_is_linked_via_previous_hashes() {
        let env = TestEnv::new();
        let (soul_id, ent_id) = seed(&env);
        update_entity(&env.conn, &ent_id, "active", None, "device_t").unwrap();
        update_entity(&env.conn, &ent_id, "candidate", None, "device_t").unwrap();

        let events = list_events(&env.conn, &soul_id).unwrap();
        assert!(events.len() >= 4);
        let mut prev: Option<String> = None;
        for ev in &events {
            assert_eq!(ev.previous_event_hash, prev);
            prev = Some(ev.content_hash.clone());
        }
    }
}
