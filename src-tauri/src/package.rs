use crate::crypto;
use crate::db;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub const PACKAGE_FORMAT: &str = "soul-package";
pub const PACKAGE_FORMAT_VERSION: &str = "0.1.0";
pub const SCHEMA_VERSION: &str = "0.1.0";
pub const PAYLOAD_FORMAT: &str = "soul-export";
pub const PAYLOAD_VERSION: &str = "1";
pub const MAX_PACKAGE_BYTES: usize = 100 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CipherParams {
    pub name: String,
    pub kdf: String,
    pub salt: String,
    pub nonce: String,
    #[serde(rename = "memCostKib")]
    pub mem_cost_kib: u32,
    #[serde(rename = "timeCost")]
    pub time_cost: u32,
    pub parallelism: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Envelope {
    pub format: String,
    pub format_version: String,
    pub schema_version: String,
    pub soul_id: String,
    pub display_name: String,
    pub device_id: String,
    pub created_at: String,
    pub content_hash: String,
    pub device_public_key: String,
    pub cipher: CipherParams,
    pub payload_ciphertext: String,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalibrationPayload {
    pub step: i32,
    pub answers: String,
    pub activated: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SoulExportPayload {
    pub format: String,
    pub version: String,
    pub soul: db::SoulManifest,
    pub entities: Vec<db::EntityRow>,
    pub events: Vec<db::SoulEvent>,
    pub calibration: CalibrationPayload,
}

#[derive(Debug, Serialize)]
pub struct ExportReceipt {
    pub path: String,
    pub soul_id: String,
    pub display_name: String,
    pub entity_count: i64,
    pub event_count: i64,
    pub content_hash: String,
    pub signature: String,
    pub size_bytes: usize,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct JsonExportReceipt {
    pub path: String,
    pub size_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct MarkdownExportReceipt {
    pub path: String,
    pub size_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct EntityCount {
    pub entity_type: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ImportPreview {
    pub soul_id: String,
    pub display_name: String,
    pub created_at: String,
    pub schema_version: String,
    pub format_version: String,
    pub entity_count: usize,
    pub event_count: usize,
    pub calibration_step: i32,
    pub activated: bool,
    pub head_event_hash: Option<String>,
    pub entity_counts: Vec<EntityCount>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletionReceipt {
    pub deleted_at: String,
    pub entity_count: i64,
    pub event_count: i64,
    pub keys_deleted: bool,
}

/// Квитанция раскрытия контекста наружу (MCP `soul.get_context`).
/// Не содержит текста задачи, query, id сущностей, claim или секретов —
/// только что случилось, когда и сколько.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DisclosureReceipt {
    pub kind: String,
    pub disclosed_at: String,
    pub client: String,
    pub entity_count: i64,
    pub token_estimate: i64,
    pub policy_version: String,
    pub state_version: String,
    pub max_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct ReceiptSummary {
    pub file: String,
    /// "deletion" | "disclosure"
    pub kind: String,
    /// Момент события (deleted_at / disclosed_at), RFC3339.
    pub at: String,
    pub entity_count: i64,
    pub event_count: Option<i64>,
    pub keys_deleted: Option<bool>,
    pub client: Option<String>,
    pub token_estimate: Option<i64>,
    pub policy_version: Option<String>,
    pub state_version: Option<String>,
}

/// Атомарная запись квитанции раскрытия в каталог receipts.
pub fn write_disclosure_receipt(app_dir: &Path, receipt: &DisclosureReceipt) -> Result<(), String> {
    let receipts_dir = app_dir.join("receipts");
    fs::create_dir_all(&receipts_dir).map_err(|e| format!("Cannot write receipt: {e}"))?;
    let path = receipts_dir.join(format!("disclosure-{}.json", Uuid::new_v4()));
    let json = serde_json::to_string_pretty(receipt).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Cannot write receipt: {e}"))
}

/// Список локальных квитанций (deletion-*.json, disclosure-*.json) из каталога
/// receipts. Повреждённые или неожиданные файлы пропускаются — одна битая
/// квитанция не роняет весь список. Сортировка: свежие первыми.
pub fn list_local_receipts(app_dir: &Path) -> Result<Vec<ReceiptSummary>, String> {
    let receipts_dir = app_dir.join("receipts");
    if !receipts_dir.exists() {
        return Ok(Vec::new());
    }
    let mut receipts = Vec::new();
    let entries =
        std::fs::read_dir(&receipts_dir).map_err(|e| format!("Cannot read receipts: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let size = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or_default();
        if size > 1_000_000 {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(r) = serde_json::from_str::<DisclosureReceipt>(&text) {
                receipts.push(ReceiptSummary {
                    file: file_name_of(&path),
                    kind: "disclosure".to_string(),
                    at: r.disclosed_at,
                    entity_count: r.entity_count,
                    event_count: None,
                    keys_deleted: None,
                    client: Some(r.client),
                    token_estimate: Some(r.token_estimate),
                    policy_version: Some(r.policy_version),
                    state_version: Some(r.state_version),
                });
                continue;
            }
            if let Ok(r) = serde_json::from_str::<DeletionReceipt>(&text) {
                receipts.push(ReceiptSummary {
                    file: file_name_of(&path),
                    kind: "deletion".to_string(),
                    at: r.deleted_at,
                    entity_count: r.entity_count,
                    event_count: Some(r.event_count),
                    keys_deleted: Some(r.keys_deleted),
                    client: None,
                    token_estimate: None,
                    policy_version: None,
                    state_version: None,
                });
            }
        }
    }
    receipts.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(receipts)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[derive(Debug)]
pub struct VerifiedPackage {
    pub payload: SoulExportPayload,
}

pub fn build_export_payload(
    conn: &rusqlite::Connection,
    soul_id: &str,
) -> Result<SoulExportPayload, String> {
    let soul = db::get_soul(conn, soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found.".to_string())?;
    let entities = db::list_entities(conn, soul_id).map_err(|e| e.to_string())?;
    let events = db::list_events(conn, soul_id).map_err(|e| e.to_string())?;
    let (step, answers, activated, _) =
        db::get_soul_state(conn, soul_id).map_err(|e| e.to_string())?;
    Ok(SoulExportPayload {
        format: PAYLOAD_FORMAT.to_string(),
        version: PAYLOAD_VERSION.to_string(),
        soul,
        entities,
        events,
        calibration: CalibrationPayload {
            step,
            answers,
            activated,
        },
    })
}

pub fn export_package(
    conn: &rusqlite::Connection,
    app_dir: &Path,
    soul_id: &str,
    password: &str,
    path: &Path,
) -> Result<ExportReceipt, String> {
    export_package_with_params(conn, app_dir, soul_id, password, path, None)
}

pub fn export_package_with_params(
    conn: &rusqlite::Connection,
    app_dir: &Path,
    soul_id: &str,
    password: &str,
    path: &Path,
    kdf: Option<(u32, u32, u32)>,
) -> Result<ExportReceipt, String> {
    crypto::ensure_password_valid(password)?;
    let payload = build_export_payload(conn, soul_id)?;
    let plaintext = serde_json::to_vec(&payload).map_err(|e| format!("Serialize failed: {e}"))?;

    let (mem_kib, time, p) = kdf.unwrap_or_else(crypto::default_kdf_params);
    let sealed = crypto::encrypt_payload(&plaintext, password, mem_kib, time, p)?;

    let content_hash = crypto::sha256_hex(&plaintext);
    let keys = crypto::ensure_device_keypair(app_dir)?;

    let mut envelope = Envelope {
        format: PACKAGE_FORMAT.to_string(),
        format_version: PACKAGE_FORMAT_VERSION.to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        soul_id: payload.soul.soul_id.clone(),
        display_name: payload.soul.display_name.clone(),
        device_id: payload.soul.device_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        content_hash,
        device_public_key: keys.public_b64.clone(),
        cipher: CipherParams {
            name: "xchacha20-poly1305".to_string(),
            kdf: "argon2id".to_string(),
            salt: B64.encode(sealed.salt),
            nonce: B64.encode(sealed.nonce),
            mem_cost_kib: mem_kib,
            time_cost: time,
            parallelism: p,
        },
        payload_ciphertext: B64.encode(&sealed.ciphertext),
        signature: None,
    };

    let canonical = serde_json::to_vec(&envelope).map_err(|e| format!("Serialize failed: {e}"))?;
    let mut to_sign = sha256_bytes(&canonical).to_vec();
    to_sign.extend_from_slice(&sealed.ciphertext);
    let signature = crypto::sign_bytes(&keys.private_bytes, &to_sign);
    envelope.signature = Some(B64.encode(signature));

    let file_bytes = serde_json::to_vec(&envelope).map_err(|e| format!("Serialize failed: {e}"))?;
    fs::write(path, &file_bytes).map_err(|e| format!("Cannot write export file: {e}"))?;

    Ok(ExportReceipt {
        path: path.to_string_lossy().to_string(),
        soul_id: payload.soul.soul_id,
        display_name: payload.soul.display_name,
        entity_count: payload.entities.len() as i64,
        event_count: payload.events.len() as i64,
        content_hash: envelope.content_hash.clone(),
        signature: envelope.signature.unwrap_or_default(),
        size_bytes: file_bytes.len(),
        created_at: envelope.created_at,
    })
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn verify_event_chain(payload: &SoulExportPayload) -> Result<(), String> {
    if payload.events.is_empty() {
        if payload.soul.head_event_hash.is_some() {
            return Err("Package head event hash is set but events are missing.".into());
        }
        return Ok(());
    }
    let mut last_hash: Option<String> = None;
    for ev in &payload.events {
        let h = db::compute_hash(&ev.payload);
        if h != ev.content_hash {
            return Err(format!(
                "Event {} content hash does not match its payload.",
                ev.event_id
            ));
        }
        last_hash = Some(h);
    }
    let head = payload
        .soul
        .head_event_hash
        .as_deref()
        .ok_or("Package event chain has no head event hash.".to_string())?;
    if last_hash.as_deref() != Some(head) {
        return Err("Package head event hash does not match the event chain.".into());
    }
    Ok(())
}

fn verify_entity_data_json(payload: &SoulExportPayload) -> Result<(), String> {
    for e in &payload.entities {
        if serde_json::from_str::<serde_json::Value>(&e.data).is_err() {
            return Err(format!("Entity {} has invalid data.", e.id));
        }
    }
    Ok(())
}

pub fn verify_package_bytes(
    bytes: &[u8],
    password: &str,
    max_bytes: usize,
) -> Result<VerifiedPackage, String> {
    if bytes.is_empty() {
        return Err("Package file is empty.".into());
    }
    if bytes.len() > max_bytes {
        return Err(format!("Package is too large (limit {max_bytes} bytes)."));
    }

    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|_| "Package is not a valid SOUL envelope.".to_string())?;

    if envelope.format != PACKAGE_FORMAT {
        return Err("Unknown package format.".into());
    }
    if envelope.format_version != PACKAGE_FORMAT_VERSION {
        return Err(format!(
            "Unsupported package format version: {}.",
            envelope.format_version
        ));
    }
    if envelope.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported schema version: {}.",
            envelope.schema_version
        ));
    }
    if envelope.cipher.name != "xchacha20-poly1305" {
        return Err("Unsupported cipher.".into());
    }
    if envelope.cipher.kdf != "argon2id" {
        return Err("Unsupported key derivation function.".into());
    }

    let signature_b64 = envelope
        .signature
        .as_deref()
        .ok_or("Package is not signed.".to_string())?;
    let signature = B64
        .decode(signature_b64)
        .map_err(|_| "Invalid signature encoding.".to_string())?;
    if signature.len() != crypto::SIG_LEN {
        return Err("Invalid signature length.".into());
    }
    let salt = B64
        .decode(&envelope.cipher.salt)
        .map_err(|_| "Invalid salt encoding.".to_string())?;
    if salt.len() != crypto::SALT_LEN {
        return Err("Invalid salt length.".into());
    }
    let nonce = B64
        .decode(&envelope.cipher.nonce)
        .map_err(|_| "Invalid nonce encoding.".to_string())?;
    if nonce.len() != crypto::NONCE_LEN {
        return Err("Invalid nonce length.".into());
    }
    let ciphertext = B64
        .decode(&envelope.payload_ciphertext)
        .map_err(|_| "Invalid payload encoding.".to_string())?;

    let mut canonical_env = envelope.clone();
    canonical_env.signature = None;
    let canonical =
        serde_json::to_vec(&canonical_env).map_err(|e| format!("Serialize failed: {e}"))?;
    let mut to_sign = sha256_bytes(&canonical).to_vec();
    to_sign.extend_from_slice(&ciphertext);
    if !crypto::verify_signature(&envelope.device_public_key, &to_sign, &signature) {
        return Err("Package signature is invalid. The file may have been modified.".into());
    }

    let plaintext = crypto::decrypt_payload(
        &ciphertext,
        password,
        &salt,
        &nonce,
        envelope.cipher.mem_cost_kib,
        envelope.cipher.time_cost,
        envelope.cipher.parallelism,
    )?;

    if crypto::sha256_hex(&plaintext) != envelope.content_hash {
        return Err("Package content hash mismatch. The file may be corrupted.".into());
    }

    let payload: SoulExportPayload = serde_json::from_slice(&plaintext)
        .map_err(|_| "Package payload is invalid.".to_string())?;
    if payload.format != PAYLOAD_FORMAT {
        return Err("Unknown payload format.".into());
    }
    if payload.version != PAYLOAD_VERSION {
        return Err(format!("Unsupported payload version: {}.", payload.version));
    }
    if payload.soul.soul_id != envelope.soul_id {
        return Err("Package soul ID does not match its manifest.".into());
    }
    verify_event_chain(&payload)?;
    verify_entity_data_json(&payload)?;

    Ok(VerifiedPackage { payload })
}

pub fn inspect_package_file(path: &Path, password: &str) -> Result<ImportPreview, String> {
    let bytes = crypto::read_file_limited(path, MAX_PACKAGE_BYTES)?;
    let vp = verify_package_bytes(&bytes, password, MAX_PACKAGE_BYTES)?;
    let mut counts: Vec<EntityCount> = Vec::new();
    for e in &vp.payload.entities {
        match counts.iter_mut().find(|c| c.entity_type == e.entity_type) {
            Some(c) => c.count += 1,
            None => counts.push(EntityCount {
                entity_type: e.entity_type.clone(),
                count: 1,
            }),
        }
    }
    Ok(ImportPreview {
        soul_id: vp.payload.soul.soul_id,
        display_name: vp.payload.soul.display_name,
        created_at: vp.payload.soul.created_at,
        schema_version: vp.payload.soul.schema_version,
        format_version: vp.payload.soul.format_version,
        entity_count: vp.payload.entities.len(),
        event_count: vp.payload.events.len(),
        calibration_step: vp.payload.calibration.step,
        activated: vp.payload.calibration.activated,
        head_event_hash: vp.payload.soul.head_event_hash,
        entity_counts: counts,
    })
}

pub fn import_package_file(
    conn: &mut rusqlite::Connection,
    path: &Path,
    password: &str,
) -> Result<db::SoulManifest, String> {
    let bytes = crypto::read_file_limited(path, MAX_PACKAGE_BYTES)?;
    let vp = verify_package_bytes(&bytes, password, MAX_PACKAGE_BYTES)?;
    let payload = vp.payload;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    db::wipe_all_tx(&tx).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO souls (soul_id, display_name, format_version, schema_version, created_at, head_event_hash, entity_count, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            payload.soul.soul_id,
            payload.soul.display_name,
            payload.soul.format_version,
            payload.soul.schema_version,
            payload.soul.created_at,
            payload.soul.head_event_hash,
            payload.entities.len() as i64,
            payload.soul.device_id
        ],
    )
    .map_err(|e| e.to_string())?;

    for ev in &payload.events {
        tx.execute(
            "INSERT INTO events (event_id, soul_id, device_id, actor, hlc, operation, entity_type, entity_id, payload, provenance_ids, previous_event_hash, content_hash, signature, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                ev.event_id,
                ev.soul_id,
                ev.device_id,
                ev.actor,
                ev.hlc,
                ev.operation,
                ev.entity_type,
                ev.entity_id,
                ev.payload,
                serde_json::to_string(&ev.provenance_ids).unwrap_or_else(|_| "[]".into()),
                ev.previous_event_hash,
                ev.content_hash,
                ev.signature,
                ev.created_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for ent in &payload.entities {
        tx.execute(
            "INSERT INTO entities (id, soul_id, entity_type, status, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                ent.id,
                ent.soul_id,
                ent.entity_type,
                ent.status,
                ent.data,
                ent.created_at,
                ent.updated_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "INSERT INTO soul_state (soul_id, activated, calibration_step, calibration_answers)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            payload.soul.soul_id,
            if payload.calibration.activated { 1 } else { 0 },
            payload.calibration.step,
            payload.calibration.answers
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    db::get_soul(conn, &payload.soul.soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("Restored SOUL could not be read back.".to_string())
}

pub fn export_json(
    conn: &rusqlite::Connection,
    soul_id: &str,
    path: &Path,
) -> Result<JsonExportReceipt, String> {
    let payload = build_export_payload(conn, soul_id)?;
    let doc = serde_json::json!({
        "exportedAt": Utc::now().to_rfc3339(),
        "format": "soul-export-json",
        "soul": payload.soul,
        "entities": payload.entities,
        "events": payload.events,
        "calibration": payload.calibration,
    });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| format!("Serialize failed: {e}"))?;
    fs::write(path, text).map_err(|e| format!("Cannot write export file: {e}"))?;
    let size = fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
    Ok(JsonExportReceipt {
        path: path.to_string_lossy().to_string(),
        size_bytes: size,
    })
}

fn claim_from_entity(e: &db::EntityRow) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&e.data) {
        if let Some(claim) = v.get("claim").and_then(|c| c.as_str()) {
            return claim.to_string();
        }
        return v.to_string();
    }
    e.data.clone()
}

pub fn export_markdown(
    conn: &rusqlite::Connection,
    soul_id: &str,
    path: &Path,
) -> Result<MarkdownExportReceipt, String> {
    let payload = build_export_payload(conn, soul_id)?;
    let mut out = String::new();

    out.push_str(&format!("# SOUL Export: {}\n\n", payload.soul.display_name));
    out.push_str(&format!("- Soul ID: `{}`\n", payload.soul.soul_id));
    out.push_str(&format!("- Created: {}\n", payload.soul.created_at));
    out.push_str(&format!(
        "- Schema version: {}\n",
        payload.soul.schema_version
    ));
    out.push_str(&format!("- Entities: {}\n", payload.entities.len()));
    out.push_str(&format!("- Events: {}\n", payload.events.len()));
    out.push_str(&format!(
        "- Calibration step: {}\n",
        payload.calibration.step
    ));
    out.push_str(&format!(
        "- Activated: {}\n",
        if payload.calibration.activated {
            "yes"
        } else {
            "no"
        }
    ));
    if let Some(head) = &payload.soul.head_event_hash {
        out.push_str(&format!("- Head event hash: `{}`\n", head));
    }
    out.push('\n');

    let mut by_type: std::collections::BTreeMap<String, Vec<&db::EntityRow>> =
        std::collections::BTreeMap::new();
    for e in &payload.entities {
        by_type.entry(e.entity_type.clone()).or_default().push(e);
    }
    for (etype, rows) in &by_type {
        out.push_str(&format!("## {}\n\n", etype));
        for e in rows {
            out.push_str(&format!(
                "- [{status}] {claim}\n",
                status = e.status,
                claim = claim_from_entity(e)
            ));
        }
        out.push('\n');
    }

    fs::write(path, &out).map_err(|e| format!("Cannot write export file: {e}"))?;
    let size = fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
    Ok(MarkdownExportReceipt {
        path: path.to_string_lossy().to_string(),
        size_bytes: size,
    })
}

pub fn wipe_local_data(
    conn: &rusqlite::Connection,
    app_dir: &Path,
    soul_id: &str,
) -> Result<DeletionReceipt, String> {
    let (entity_count, event_count) = db::count_soul(conn, soul_id).map_err(|e| e.to_string())?;
    db::wipe_all(conn).map_err(|e| e.to_string())?;

    let keys_deleted = crypto::delete_device_keys(app_dir).is_ok();

    let receipt = DeletionReceipt {
        deleted_at: Utc::now().to_rfc3339(),
        entity_count,
        event_count,
        keys_deleted,
    };
    let receipts_dir = app_dir.join("receipts");
    fs::create_dir_all(&receipts_dir).map_err(|e| format!("Cannot write receipt: {e}"))?;
    let receipt_path = receipts_dir.join(format!("deletion-{}.json", Uuid::new_v4()));
    let receipt_json = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    fs::write(&receipt_path, receipt_json).map_err(|e| format!("Cannot write receipt: {e}"))?;

    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;
    use crate::db::{
        activate_soul, add_entity, confirm_soul_preview, create_soul, get_calibration, init_db,
        is_soul_activated, list_entities, list_events, list_souls, save_calibration,
    };
    use rusqlite::Connection;
    use std::path::PathBuf;

    const PASSWORD: &str = "test-passphrase-123";
    const FAST_KDF: (u32, u32, u32) = (1024, 1, 1);

    struct TestEnv {
        dir: PathBuf,
        conn: Connection,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-test-{}", Uuid::new_v4()));
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

    fn create_seeded_soul(env: &mut TestEnv) -> String {
        let soul = create_soul(&env.conn, "Тест Илья", "device_t1").unwrap();
        add_entity(
            &env.conn,
            &soul.soul_id,
            "preference",
            "candidate",
            r#"{"claim":"Prefer concise answers","source":"calibration"}"#,
            "device_t1",
        )
        .unwrap();
        add_entity(
            &env.conn,
            &soul.soul_id,
            "boundary",
            "candidate",
            r#"{"claim":"Never share financial data"}"#,
            "device_t1",
        )
        .unwrap();
        save_calibration(
            &env.conn,
            &soul.soul_id,
            2,
            r#"[{"questionId":"q1","value":"yes"}]"#,
        )
        .unwrap();
        confirm_soul_preview(&env.conn, &soul.soul_id, "device_t1").unwrap();
        activate_soul(&env.conn, &soul.soul_id, "device_t1").unwrap();
        soul.soul_id
    }

    fn export_fast(env: &TestEnv, soul_id: &str) -> PathBuf {
        let path = env.dir.join("backup.soul");
        export_package_with_params(
            &env.conn,
            &env.dir,
            soul_id,
            PASSWORD,
            &path,
            Some(FAST_KDF),
        )
        .unwrap();
        path
    }

    /// Собирает подписанный пакет из произвольного payload (для тестов,
    /// где экспорт из БД невозможен).
    fn package_from_payload(env: &TestEnv, payload: &SoulExportPayload) -> Vec<u8> {
        let plaintext = serde_json::to_vec(payload).unwrap();
        let (mem_kib, time, p) = FAST_KDF;
        let sealed = crypto::encrypt_payload(&plaintext, PASSWORD, mem_kib, time, p).unwrap();
        let keys = crypto::ensure_device_keypair(&env.dir).unwrap();
        let mut envelope = Envelope {
            format: PACKAGE_FORMAT.to_string(),
            format_version: PACKAGE_FORMAT_VERSION.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            soul_id: payload.soul.soul_id.clone(),
            display_name: payload.soul.display_name.clone(),
            device_id: payload.soul.device_id.clone(),
            created_at: Utc::now().to_rfc3339(),
            content_hash: crypto::sha256_hex(&plaintext),
            device_public_key: keys.public_b64.clone(),
            cipher: CipherParams {
                name: "xchacha20-poly1305".to_string(),
                kdf: "argon2id".to_string(),
                salt: B64.encode(sealed.salt),
                nonce: B64.encode(sealed.nonce),
                mem_cost_kib: mem_kib,
                time_cost: time,
                parallelism: p,
            },
            payload_ciphertext: B64.encode(&sealed.ciphertext),
            signature: None,
        };
        let canonical = serde_json::to_vec(&envelope).unwrap();
        let mut to_sign = sha256_bytes(&canonical).to_vec();
        to_sign.extend_from_slice(&sealed.ciphertext);
        let signature = crypto::sign_bytes(&keys.private_bytes, &to_sign);
        envelope.signature = Some(B64.encode(signature));
        serde_json::to_vec(&envelope).unwrap()
    }

    #[test]
    fn envelope_canonical_serialization_is_stable() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let _ = soul_id;
        let keys = crypto::ensure_device_keypair(&env.dir).unwrap();
        let envelope = Envelope {
            format: PACKAGE_FORMAT.to_string(),
            format_version: PACKAGE_FORMAT_VERSION.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            soul_id: "soul_x".to_string(),
            display_name: "Тест Я с юникодом ✓".to_string(),
            device_id: "device_1".to_string(),
            created_at: "2026-07-31T10:00:00Z".to_string(),
            content_hash: "abc123".to_string(),
            device_public_key: keys.public_b64,
            cipher: CipherParams {
                name: "xchacha20-poly1305".to_string(),
                kdf: "argon2id".to_string(),
                salt: B64.encode([7u8; crypto::SALT_LEN]),
                nonce: B64.encode([9u8; crypto::NONCE_LEN]),
                mem_cost_kib: 1024,
                time_cost: 1,
                parallelism: 1,
            },
            payload_ciphertext: "cipher".to_string(),
            signature: None,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let parsed: Envelope = serde_json::from_slice(&bytes).unwrap();
        let again = serde_json::to_vec(&parsed).unwrap();
        assert_eq!(bytes, again);
    }

    #[test]
    fn export_roundtrip_verifies() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let vp = verify_package_bytes(&bytes, PASSWORD, MAX_PACKAGE_BYTES).unwrap();
        assert_eq!(vp.payload.soul.soul_id, soul_id);
        assert_eq!(vp.payload.entities.len(), 2);
        assert_eq!(vp.payload.calibration.step, 2);
        assert!(vp.payload.calibration.activated);
        assert!(vp.payload.events.len() >= 3);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let err = verify_package_bytes(&bytes, "wrong-passphrase", MAX_PACKAGE_BYTES).unwrap_err();
        assert!(
            err.contains("Incorrect passphrase"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
        let ct = B64.decode(&envelope.payload_ciphertext).unwrap();
        let mut tampered = ct;
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0x01;
        envelope.payload_ciphertext = B64.encode(&tampered);
        let err = verify_package_bytes(
            &serde_json::to_vec(&envelope).unwrap(),
            PASSWORD,
            MAX_PACKAGE_BYTES,
        )
        .unwrap_err();
        assert!(err.contains("signature"), "unexpected error: {err}");
    }

    #[test]
    fn tampered_manifest_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
        envelope.display_name = "Attacker".to_string();
        let err = verify_package_bytes(
            &serde_json::to_vec(&envelope).unwrap(),
            PASSWORD,
            MAX_PACKAGE_BYTES,
        )
        .unwrap_err();
        assert!(err.contains("signature"), "unexpected error: {err}");
    }

    #[test]
    fn corrupted_json_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let truncated = &bytes[..bytes.len() / 2];
        let err = verify_package_bytes(truncated, PASSWORD, MAX_PACKAGE_BYTES).unwrap_err();
        assert!(
            err.contains("not a valid SOUL envelope"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn unknown_versions_are_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();

        let mut env_bad_format: Envelope = serde_json::from_slice(&bytes).unwrap();
        env_bad_format.format_version = "9.9.9".to_string();
        let err = verify_package_bytes(
            &serde_json::to_vec(&env_bad_format).unwrap(),
            PASSWORD,
            MAX_PACKAGE_BYTES,
        )
        .unwrap_err();
        assert!(err.contains("format version"), "unexpected error: {err}");

        let mut env_bad_schema: Envelope = serde_json::from_slice(&bytes).unwrap();
        env_bad_schema.schema_version = "0.2.0".to_string();
        let err = verify_package_bytes(
            &serde_json::to_vec(&env_bad_schema).unwrap(),
            PASSWORD,
            MAX_PACKAGE_BYTES,
        )
        .unwrap_err();
        assert!(err.contains("schema version"), "unexpected error: {err}");
    }

    #[test]
    fn unsigned_package_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
        envelope.signature = None;
        let err = verify_package_bytes(
            &serde_json::to_vec(&envelope).unwrap(),
            PASSWORD,
            MAX_PACKAGE_BYTES,
        )
        .unwrap_err();
        assert!(err.contains("not signed"), "unexpected error: {err}");
    }

    #[test]
    fn oversize_package_is_rejected() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let err = verify_package_bytes(&bytes, PASSWORD, 100).unwrap_err();
        assert!(err.contains("too large"), "unexpected error: {err}");
    }

    #[test]
    fn import_restores_into_fresh_storage() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);

        let preview = inspect_package_file(&path, PASSWORD).unwrap();
        assert_eq!(preview.soul_id, soul_id);
        assert_eq!(preview.entity_count, 2);
        assert_eq!(preview.calibration_step, 2);
        assert!(preview.activated);
        assert_eq!(preview.entity_counts.len(), 2);

        let restored = import_package_file(&mut env.conn, &path, PASSWORD).unwrap();
        assert_eq!(restored.soul_id, soul_id);
        assert_eq!(restored.entity_count, 2);
        assert_eq!(get_calibration(&env.conn, &soul_id).unwrap().0, 2);
        assert!(is_soul_activated(&env.conn, &soul_id).unwrap());
        let entities = list_entities(&env.conn, &soul_id).unwrap();
        assert_eq!(entities.len(), 2);
        assert!(entities.iter().any(|e| e.entity_type == "boundary"));
        let events = list_events(&env.conn, &soul_id).unwrap();
        assert!(events.len() >= 3);
    }

    #[test]
    fn failed_import_with_invalid_payload_preserves_existing_soul() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let vp = verify_package_bytes(&bytes, PASSWORD, MAX_PACKAGE_BYTES).unwrap();

        // Валидно подписанный пакет с дублирующимся id сущности: проверка
        // подписи проходит, но вставка падает (UNIQUE) — существующий SOUL
        // обязан уцелеть.
        let mut payload = vp.payload.clone();
        payload.entities.push(payload.entities[0].clone());
        let bad_path = env.dir.join("dup-events.soul");
        std::fs::write(&bad_path, package_from_payload(&env, &payload)).unwrap();

        let err = import_package_file(&mut env.conn, &bad_path, PASSWORD).unwrap_err();
        assert!(err.contains("UNIQUE"), "unexpected error: {err}");
        assert_eq!(
            list_souls(&env.conn).unwrap().len(),
            1,
            "existing soul must survive failed import"
        );
        assert_eq!(list_entities(&env.conn, &soul_id).unwrap().len(), 2);
        assert!(is_soul_activated(&env.conn, &soul_id).unwrap());
    }

    #[test]
    fn failed_import_leaves_storage_unchanged() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = export_fast(&env, &soul_id);
        let bytes = std::fs::read(&path).unwrap();
        let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
        let ct = B64.decode(&envelope.payload_ciphertext).unwrap();
        let mut tampered = ct;
        tampered[3] ^= 0x01;
        envelope.payload_ciphertext = B64.encode(&tampered);
        let tampered_path = env.dir.join("tampered.soul");
        std::fs::write(&tampered_path, serde_json::to_vec(&envelope).unwrap()).unwrap();

        let err = import_package_file(&mut env.conn, &tampered_path, PASSWORD).unwrap_err();
        assert!(err.contains("signature"), "unexpected error: {err}");
        assert_eq!(
            list_souls(&env.conn).unwrap().len(),
            1,
            "storage must not change after failed import"
        );
    }

    #[test]
    fn wipe_removes_data_keys_and_writes_receipt() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let _ = export_fast(&env, &soul_id);
        assert!(crypto::keys_dir(&env.dir).exists());

        let receipt = wipe_local_data(&env.conn, &env.dir, &soul_id).unwrap();
        assert_eq!(receipt.entity_count, 2);
        assert!(receipt.keys_deleted);

        assert!(list_souls(&env.conn).unwrap().is_empty());
        assert!(list_entities(&env.conn, &soul_id).unwrap().is_empty());
        assert!(!crypto::keys_dir(&env.dir).exists());
        let receipts_dir = env.dir.join("receipts");
        assert!(receipts_dir.exists());
        let files: Vec<_> = std::fs::read_dir(&receipts_dir).unwrap().collect();
        assert_eq!(files.len(), 1);

        let fresh = init_db(&env.dir).unwrap();
        assert!(list_souls(&fresh).unwrap().is_empty());
    }

    #[test]
    fn list_receipts_returns_empty_when_no_receipts_dir() {
        let env = TestEnv::new();
        assert!(list_local_receipts(&env.dir).unwrap().is_empty());
    }

    #[test]
    fn list_receipts_lists_wipe_receipts_sorted_and_skips_corrupted() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        wipe_local_data(&env.conn, &env.dir, &soul_id).unwrap();

        let receipts_dir = env.dir.join("receipts");
        std::fs::write(receipts_dir.join("deletion-corrupted.json"), "not json").unwrap();
        std::fs::write(receipts_dir.join("notes.txt"), "ignore me").unwrap();

        let receipts = list_local_receipts(&env.dir).unwrap();
        assert_eq!(receipts.len(), 1);
        let r = &receipts[0];
        assert!(r.file.starts_with("deletion-"));
        assert_eq!(r.kind, "deletion");
        assert_eq!(r.entity_count, 2);
        assert_eq!(r.keys_deleted, Some(true));
    }

    #[test]
    fn list_receipts_sorts_by_deleted_at_desc() {
        let env = TestEnv::new();
        let receipts_dir = env.dir.join("receipts");
        std::fs::create_dir_all(&receipts_dir).unwrap();
        for (name, ts) in [
            ("deletion-old.json", "2026-01-01T00:00:00Z"),
            ("deletion-new.json", "2026-07-31T00:00:00Z"),
        ] {
            let body = format!(
                r#"{{"deleted_at":"{ts}","entity_count":1,"event_count":2,"keys_deleted":true}}"#
            );
            std::fs::write(receipts_dir.join(name), body).unwrap();
        }
        let receipts = list_local_receipts(&env.dir).unwrap();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].file, "deletion-new.json");
        assert_eq!(receipts[1].file, "deletion-old.json");
    }

    #[test]
    fn list_receipts_skips_oversized_files() {
        let env = TestEnv::new();
        let receipts_dir = env.dir.join("receipts");
        std::fs::create_dir_all(&receipts_dir).unwrap();
        let mut body = String::new();
        body.push_str(
            r#"{"deleted_at":"2026-01-01T00:00:00Z","entity_count":1,"event_count":1,"keys_deleted":true}"#,
        );
        body.push_str(&" ".repeat(1_100_000));
        std::fs::write(receipts_dir.join("deletion-big.json"), body).unwrap();
        assert!(list_local_receipts(&env.dir).unwrap().is_empty());
    }

    #[test]
    fn weak_passphrase_is_rejected_on_export() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path = env.dir.join("weak.soul");
        let err = export_package_with_params(
            &env.conn,
            &env.dir,
            &soul_id,
            "short",
            &path,
            Some(FAST_KDF),
        )
        .unwrap_err();
        assert!(err.contains("8 characters"), "unexpected error: {err}");
        assert!(!path.exists());
    }

    #[test]
    fn export_requires_existing_soul() {
        let env = TestEnv::new();
        let path = env.dir.join("missing.soul");
        let err = export_package_with_params(
            &env.conn,
            &env.dir,
            "soul_nonexistent",
            PASSWORD,
            &path,
            Some(FAST_KDF),
        )
        .unwrap_err();
        assert!(err.contains("SOUL not found"), "unexpected error: {err}");
    }

    #[test]
    fn reimport_is_idempotent() {
        let mut env = TestEnv::new();
        let soul_id = create_seeded_soul(&mut env);
        let path1 = export_fast(&env, &soul_id);
        import_package_file(&mut env.conn, &path1, PASSWORD).unwrap();
        let path2 = env.dir.join("backup2.soul");
        export_package_with_params(
            &env.conn,
            &env.dir,
            &soul_id,
            PASSWORD,
            &path2,
            Some(FAST_KDF),
        )
        .unwrap();
        let bytes2 = std::fs::read(&path2).unwrap();
        let vp = verify_package_bytes(&bytes2, PASSWORD, MAX_PACKAGE_BYTES).unwrap();
        assert_eq!(vp.payload.entities.len(), 2);
        assert_eq!(
            vp.payload.events.len(),
            std::fs::read(&path1)
                .map(|b| {
                    verify_package_bytes(&b, PASSWORD, MAX_PACKAGE_BYTES)
                        .unwrap()
                        .payload
                        .events
                        .len()
                })
                .unwrap()
        );
    }
}
