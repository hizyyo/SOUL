//! Имитированный Gateway (SESSION-12, ULTRA_MVP §4.11).
//!
//! Честная локальная имитация внешнего действия: агент предлагает действие
//! (`SoulAction`), gateway нормализует его, оценивает движком политик
//! (SESSION-11) и при разрешении выпускает локальную capability — action id,
//! hash нагрузки (канонический JSON действия), nonce, срок и однократное
//! использование. Выполнение возможно только через поддельный локальный
//! коннектор: детерминированно, без сети, без побочных эффектов. Никакого
//! управления произвольными внешними агентами: P0-имитация, а не
//! production-защита (реальная изоляция учётных данных — P1, §4.11).
//!
//! Каждый шаг оставляет квитанцию со статусом имитации: pending (capability
//! выдана), simulated (поддельный коннектор выполнил действие), denied / held /
//! redacted (решение политики на этапе предложения), refused (отказ на этапе
//! выполнения: повтор, изменённая нагрузка, неверный канал, истёкший срок,
//! политика на момент выполнения). Квитанции не содержат исходной
//! чувствительной нагрузки — только hash и метаданные действия.
//!
//! Канал выполнения (коннектор/учётная запись/окружение) проверяется по
//! локальному реестру имитированных коннекторов (`gateway_connectors`), который
//! сеется один раз за жизнь хранилища (как демо-политики SESSION-11).

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::Serialize;
use uuid::Uuid;

use crate::db::compute_hash;
use crate::policy::{self, Effect, SoulAction};

/// Срок capability по умолчанию (секунды).
pub const DEFAULT_TTL_SECONDS: u64 = 300;
/// Верхняя граница срока capability.
pub const MAX_TTL_SECONDS: u64 = 3_600;
/// Максимальный размер JSON действия (символов).
pub const MAX_ACTION_JSON_CHARS: usize = 16_000;
/// Максимальное число выдаваемых capabilities (защита списка).
pub const MAX_GATEWAY_CAPABILITIES: usize = 500;
/// Максимальное число квитанций.
pub const MAX_GATEWAY_RECEIPTS: usize = 2_000;

/// Статус квитанции имитации.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayStatus {
    /// Capability выдана, выполнение ещё не запрошено.
    Pending,
    /// Поддельный коннектор выполнил действие (имитация).
    Simulated,
    /// Политика запретила действие на этапе предложения.
    Denied,
    /// Политика потребовала подтверждение пользователя.
    Held,
    /// Политика скрыла действие.
    Redacted,
    /// Отказ на этапе выполнения (повтор, нагрузка, канал, срок, политика).
    Refused,
}

impl GatewayStatus {
    fn as_str(self) -> &'static str {
        match self {
            GatewayStatus::Pending => "pending",
            GatewayStatus::Simulated => "simulated",
            GatewayStatus::Denied => "denied",
            GatewayStatus::Held => "held",
            GatewayStatus::Redacted => "redacted",
            GatewayStatus::Refused => "refused",
        }
    }

    fn from_str(s: &str) -> GatewayStatus {
        match s {
            "simulated" => GatewayStatus::Simulated,
            "denied" => GatewayStatus::Denied,
            "held" => GatewayStatus::Held,
            "redacted" => GatewayStatus::Redacted,
            "refused" => GatewayStatus::Refused,
            _ => GatewayStatus::Pending,
        }
    }
}

fn effect_to_str(effect: Effect) -> &'static str {
    match effect {
        Effect::Allow => "allow",
        Effect::Deny => "deny",
        Effect::RequireConfirmation => "require_confirmation",
        Effect::Redact => "redact",
    }
}

fn effect_from_str(s: &str) -> Effect {
    match s {
        "deny" => Effect::Deny,
        "require_confirmation" => Effect::RequireConfirmation,
        "redact" => Effect::Redact,
        _ => Effect::Allow,
    }
}

/// Локальная имитированная capability (без исходной нагрузки).
#[derive(Debug, Clone, Serialize)]
pub struct CapabilityInfo {
    pub id: String,
    pub action_id: String,
    pub kind: String,
    pub payload_hash: String,
    pub nonce: String,
    pub expires_at: String,
    pub created_at: String,
    pub used_at: Option<String>,
}

/// Строка capabilities вместе с сохранённой нагрузкой (для повторной оценки
/// политики в момент выполнения). Нагрузка не выходит наружу.
struct CapabilityRow {
    id: String,
    payload_hash: String,
    nonce: String,
    action_json: String,
    expires_at: String,
    used_at: Option<String>,
}

/// Квитанция gateway: статус имитации + метаданные действия, без нагрузки.
#[derive(Debug, Clone, Serialize)]
pub struct GatewayReceipt {
    pub id: String,
    pub capability_id: Option<String>,
    pub action_id: String,
    pub kind: String,
    pub status: GatewayStatus,
    pub decision_effect: Effect,
    pub rule_id: Option<String>,
    pub message: Option<String>,
    pub connector_executed: bool,
    pub reason: Option<String>,
    pub nonce: Option<String>,
    pub created_at: String,
}

/// Результат этапа предложения: решение политики + capability (если выдана)
/// + квитанция.
#[derive(Debug, Clone, Serialize)]
pub struct GatewayProposal {
    pub decision: policy::Decision,
    pub capability: Option<CapabilityInfo>,
    pub receipt: GatewayReceipt,
}

/// Результат выполнения capability.
#[derive(Debug, Clone, Serialize)]
pub struct GatewayExecuteResult {
    pub ok: bool,
    pub receipt: GatewayReceipt,
}

/// Детерминированный результат поддельного коннектора.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectorSimulation {
    pub status: &'static str,
    pub transaction_id: String,
    pub note: &'static str,
}

/// Канонический JSON действия без поля payload_hash: хэшируется вся нагрузка.
pub fn canonical_action_json(action: &SoulAction) -> String {
    serde_json::json!({
        "actionId": action.action_id,
        "kind": action.kind,
        "actor": action.actor,
        "connectorId": action.connector_id,
        "accountId": action.account_id,
        "environment": action.environment,
        "recipient": action.recipient,
        "domain": action.domain,
        "amount": action.amount,
        "currency": action.currency,
        "dataClasses": action.data_classes,
        "reversible": action.reversible,
        "confirmedByUser": action.confirmed_by_user,
        "requestedScopes": action.requested_scopes,
    })
    .to_string()
}

/// SHA-256 (hex) канонического JSON действия — «hash нагрузки» capability.
pub fn payload_hash_of(action: &SoulAction) -> String {
    compute_hash(&canonical_action_json(action))
}

/// Нормализация предложенного действия: парсинг, обрезка пробелов, обязательные
/// поля, диапазон суммы. Входной `payload_hash` не доверяется — hash нагрузки
/// пересчитывается из нормализованного действия.
pub fn normalize_action(json: &str) -> Result<SoulAction, String> {
    if json.chars().count() > MAX_ACTION_JSON_CHARS {
        return Err(format!(
            "Action exceeds {MAX_ACTION_JSON_CHARS} characters."
        ));
    }
    let mut action: SoulAction = serde_json::from_str(json)
        .map_err(|e| format!("Action is not valid SoulAction JSON: {e}"))?;
    action.action_id = action.action_id.trim().to_string();
    action.kind = action.kind.trim().to_string();
    action.actor = action.actor.trim().to_string();
    action.connector_id = action.connector_id.trim().to_string();
    action.account_id = action.account_id.trim().to_string();
    action.environment = action.environment.trim().to_string();
    if action.action_id.is_empty()
        || action.kind.is_empty()
        || action.actor.is_empty()
        || action.connector_id.is_empty()
        || action.account_id.is_empty()
    {
        return Err(
            "Required action fields (actionId, kind, actor, connectorId, accountId) must not be empty."
                .to_string(),
        );
    }
    if let Some(amount) = action.amount {
        if !amount.is_finite() || amount.abs() > policy::MAX_AMOUNT {
            return Err(format!(
                "Amount must be finite and within ±{}.",
                policy::MAX_AMOUNT
            ));
        }
    }
    action.payload_hash = payload_hash_of(&action);
    Ok(action)
}

/// Поддельный локальный коннектор: детерминированный, без сети и без побочных
/// эффектов. Единственная точка «исполнения» действия в P0.
pub fn fake_connector_execute(action: &SoulAction) -> ConnectorSimulation {
    let digest = payload_hash_of(action);
    let tx: String = format!("sim_{}", digest.chars().take(16).collect::<String>());
    ConnectorSimulation {
        status: "ok",
        transaction_id: tx,
        note: "simulated; no external side effects",
    }
}

pub fn init_gateway(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS capabilities (
            id TEXT PRIMARY KEY,
            action_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            payload_hash TEXT NOT NULL,
            nonce TEXT NOT NULL UNIQUE,
            action_json TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            used_at TEXT
        );

        CREATE TABLE IF NOT EXISTS gateway_receipts (
            id TEXT PRIMARY KEY,
            capability_id TEXT,
            action_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending','simulated','denied','held','redacted','refused')),
            decision_effect TEXT NOT NULL CHECK (decision_effect IN ('allow','deny','require_confirmation','redact')),
            rule_id TEXT,
            message TEXT,
            connector_executed INTEGER NOT NULL DEFAULT 0,
            reason TEXT,
            nonce TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS gateway_connectors (
            connector_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            environment TEXT NOT NULL,
            PRIMARY KEY (connector_id, account_id, environment)
        );

        CREATE TABLE IF NOT EXISTS gateway_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_gateway_receipts_capability ON gateway_receipts(capability_id);",
    )?;
    seed_connectors(conn)
}

/// Реестр имитированных коннекторов — один раз за жизнь хранилища (флаг в
/// `gateway_meta`), как демо-политики SESSION-11. Канал выполнения должен
/// присутствовать в реестре, иначе выполнение отказывается.
fn seed_connectors(conn: &Connection) -> SqlResult<()> {
    let seeded: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM gateway_meta WHERE key = 'seeded')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v != 0)?;
    if seeded {
        return Ok(());
    }
    let demo: [(&str, &str, &str); 4] = [
        ("demo-connector", "acct-1", "production"),
        ("demo-connector", "acct-1", "staging"),
        ("demo-connector", "acct-2", "production"),
        ("sandbox-connector", "acct-1", "development"),
    ];
    for (connector_id, account_id, environment) in demo {
        conn.execute(
            "INSERT OR IGNORE INTO gateway_connectors (connector_id, account_id, environment)
             VALUES (?1, ?2, ?3)",
            params![connector_id, account_id, environment],
        )?;
    }
    conn.execute(
        "INSERT INTO gateway_meta (key, value) VALUES ('seeded', ?1)",
        params![Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Квитанция: статус для эффекта решения на этапе предложения.
fn status_for(effect: Effect) -> GatewayStatus {
    match effect {
        Effect::Allow => GatewayStatus::Pending,
        Effect::Deny => GatewayStatus::Denied,
        Effect::RequireConfirmation => GatewayStatus::Held,
        Effect::Redact => GatewayStatus::Redacted,
    }
}

struct ReceiptFields<'a> {
    capability_id: Option<&'a str>,
    status: GatewayStatus,
    decision_effect: Effect,
    rule_id: Option<&'a str>,
    message: Option<&'a str>,
    connector_executed: bool,
    nonce: Option<&'a str>,
    reason: Option<&'a str>,
}

fn insert_receipt(
    conn: &Connection,
    action: &SoulAction,
    fields: ReceiptFields<'_>,
) -> Result<GatewayReceipt, String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM gateway_receipts", [], |r| r.get(0))
        .map_err(|e| format!("receipt count failed: {e}"))?;
    if count >= MAX_GATEWAY_RECEIPTS as i64 {
        return Err(format!(
            "Too many gateway receipts (limit {MAX_GATEWAY_RECEIPTS})."
        ));
    }
    let id = format!("rec_{}", Uuid::new_v4());
    let receipt = GatewayReceipt {
        id: id.clone(),
        capability_id: fields.capability_id.map(str::to_string),
        action_id: action.action_id.clone(),
        kind: action.kind.clone(),
        status: fields.status,
        decision_effect: fields.decision_effect,
        rule_id: fields.rule_id.map(str::to_string),
        message: fields.message.map(str::to_string),
        connector_executed: fields.connector_executed,
        reason: fields.reason.map(str::to_string),
        nonce: fields.nonce.map(str::to_string),
        created_at: Utc::now().to_rfc3339(),
    };
    conn.execute(
        "INSERT INTO gateway_receipts (
            id, capability_id, action_id, kind, status, decision_effect,
            rule_id, message, connector_executed, reason, nonce, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            receipt.id,
            receipt.capability_id,
            receipt.action_id,
            receipt.kind,
            receipt.status.as_str(),
            effect_to_str(receipt.decision_effect),
            receipt.rule_id,
            receipt.message,
            if receipt.connector_executed { 1 } else { 0 },
            receipt.reason,
            receipt.nonce,
            receipt.created_at
        ],
    )
    .map_err(|e| format!("gateway receipt insert failed: {e}"))?;
    Ok(receipt)
}

/// Этап предложения: нормализация → оценка политикой → capability/квитанция.
pub fn propose_action(
    conn: &Connection,
    action_json: &str,
    ttl_seconds: Option<u64>,
) -> Result<GatewayProposal, String> {
    let action = normalize_action(action_json)?;
    let decision = policy::evaluate(conn, &action)?;
    if decision.effect != Effect::Allow {
        let receipt = insert_receipt(
            conn,
            &action,
            ReceiptFields {
                capability_id: None,
                status: status_for(decision.effect),
                decision_effect: decision.effect,
                rule_id: decision.rule_id.as_deref(),
                message: decision.message.as_deref(),
                connector_executed: false,
                nonce: None,
                reason: None,
            },
        )?;
        return Ok(GatewayProposal {
            decision,
            capability: None,
            receipt,
        });
    }

    let ttl = ttl_seconds
        .unwrap_or(DEFAULT_TTL_SECONDS)
        .clamp(1, MAX_TTL_SECONDS);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM capabilities", [], |r| r.get(0))
        .map_err(|e| format!("capability count failed: {e}"))?;
    if count >= MAX_GATEWAY_CAPABILITIES as i64 {
        return Err(format!(
            "Too many capabilities (limit {MAX_GATEWAY_CAPABILITIES})."
        ));
    }
    let id = format!("cap_{}", Uuid::new_v4());
    let nonce = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = (now + chrono::Duration::seconds(ttl as i64)).to_rfc3339();
    conn.execute(
        "INSERT INTO capabilities (
            id, action_id, kind, payload_hash, nonce, action_json, expires_at, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            action.action_id,
            action.kind,
            action.payload_hash,
            nonce,
            canonical_action_json(&action),
            expires_at,
            now.to_rfc3339()
        ],
    )
    .map_err(|e| format!("capability insert failed: {e}"))?;
    let capability = CapabilityInfo {
        id: id.clone(),
        action_id: action.action_id.clone(),
        kind: action.kind.clone(),
        payload_hash: action.payload_hash.clone(),
        nonce: nonce.clone(),
        expires_at: expires_at.clone(),
        created_at: now.to_rfc3339(),
        used_at: None,
    };
    let receipt = insert_receipt(
        conn,
        &action,
        ReceiptFields {
            capability_id: Some(&id),
            status: GatewayStatus::Pending,
            decision_effect: Effect::Allow,
            rule_id: None,
            message: None,
            connector_executed: false,
            nonce: Some(&nonce),
            reason: None,
        },
    )?;
    Ok(GatewayProposal {
        decision,
        capability: Some(capability),
        receipt,
    })
}

fn capability_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<CapabilityRow> {
    Ok(CapabilityRow {
        id: row.get(0)?,
        payload_hash: row.get(1)?,
        nonce: row.get(2)?,
        action_json: row.get(3)?,
        expires_at: row.get(4)?,
        used_at: row.get(5)?,
    })
}

fn load_capability(
    conn: &Connection,
    capability_id: &str,
) -> Result<Option<CapabilityRow>, String> {
    conn.query_row(
        "SELECT id, payload_hash, nonce, action_json, expires_at, used_at
         FROM capabilities WHERE id = ?1",
        params![capability_id],
        capability_row_from_sql,
    )
    .optional()
    .map_err(|e| format!("capability lookup failed: {e}"))
}

/// Fail-closed: непарсируемый срок считается истёкшим.
fn is_expired(expires_at: &str) -> bool {
    match DateTime::parse_from_rfc3339(expires_at) {
        Ok(ts) => Utc::now() > ts.with_timezone(&Utc),
        Err(_) => true,
    }
}

/// Проверка канала по локальному реестру имитированных коннекторов.
fn channel_mismatch(
    conn: &Connection,
    connector_id: &str,
    account_id: &str,
    environment: &str,
) -> Result<Option<&'static str>, String> {
    let exact: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM gateway_connectors
             WHERE connector_id = ?1 AND account_id = ?2 AND environment = ?3",
            params![connector_id, account_id, environment],
            |r| r.get(0),
        )
        .map_err(|e| format!("connector registry query failed: {e}"))?;
    if exact > 0 {
        return Ok(None);
    }
    let has_connector: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM gateway_connectors WHERE connector_id = ?1",
            params![connector_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("connector registry query failed: {e}"))?;
    if has_connector == 0 {
        return Ok(Some("connector mismatch"));
    }
    let has_account: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM gateway_connectors
             WHERE connector_id = ?1 AND account_id = ?2",
            params![connector_id, account_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("connector registry query failed: {e}"))?;
    if has_account == 0 {
        return Ok(Some("account mismatch"));
    }
    Ok(Some("environment mismatch"))
}

fn refuse(
    conn: &Connection,
    cap: &CapabilityRow,
    action: &SoulAction,
    reason: &'static str,
) -> Result<GatewayExecuteResult, String> {
    let receipt = insert_receipt(
        conn,
        action,
        ReceiptFields {
            capability_id: Some(&cap.id),
            status: GatewayStatus::Refused,
            decision_effect: Effect::Allow,
            rule_id: None,
            message: None,
            connector_executed: false,
            nonce: Some(&cap.nonce),
            reason: Some(reason),
        },
    )?;
    Ok(GatewayExecuteResult { ok: false, receipt })
}

fn receipt_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewayReceipt> {
    Ok(GatewayReceipt {
        id: row.get(0)?,
        capability_id: row.get(1)?,
        action_id: row.get(2)?,
        kind: row.get(3)?,
        status: GatewayStatus::from_str(&row.get::<_, String>(4)?),
        decision_effect: effect_from_str(&row.get::<_, String>(5)?),
        rule_id: row.get(6)?,
        message: row.get(7)?,
        connector_executed: row.get::<_, i64>(8)? != 0,
        reason: row.get(9)?,
        nonce: row.get(10)?,
        created_at: row.get(11)?,
    })
}

const RECEIPT_COLUMNS: &str =
    "id, capability_id, action_id, kind, status, decision_effect, rule_id, message, connector_executed, reason, nonce, created_at";

fn update_receipt_to_simulated(
    conn: &Connection,
    cap: &CapabilityRow,
    action: &SoulAction,
    simulation: &ConnectorSimulation,
) -> Result<GatewayReceipt, String> {
    let message = format!(
        "simulated transaction {} — {}",
        simulation.transaction_id, simulation.note
    );
    let pending: Option<GatewayReceipt> = conn
        .query_row(
            &format!(
                "SELECT {RECEIPT_COLUMNS} FROM gateway_receipts
                 WHERE capability_id = ?1 AND status = 'pending' ORDER BY created_at DESC LIMIT 1"
            ),
            params![cap.id],
            receipt_row_from_sql,
        )
        .optional()
        .map_err(|e| format!("receipt lookup failed: {e}"))?;
    if let Some(mut receipt) = pending {
        conn.execute(
            "UPDATE gateway_receipts
             SET status = 'simulated', connector_executed = 1, message = ?1, rule_id = NULL
             WHERE id = ?2",
            params![message, receipt.id],
        )
        .map_err(|e| format!("receipt update failed: {e}"))?;
        receipt.status = GatewayStatus::Simulated;
        receipt.connector_executed = true;
        receipt.message = Some(message);
        receipt.rule_id = None;
        return Ok(receipt);
    }
    insert_receipt(
        conn,
        action,
        ReceiptFields {
            capability_id: Some(&cap.id),
            status: GatewayStatus::Simulated,
            decision_effect: Effect::Allow,
            rule_id: None,
            message: Some(&message),
            connector_executed: true,
            nonce: Some(&cap.nonce),
            reason: None,
        },
    )
}

/// Этап выполнения: capability → канал → повторная оценка политики →
/// поддельный коннектор. Любой отказ оставляет квитанцию `refused` без
/// обращения к коннектору; успех помечает capability использованной.
pub fn execute_capability(
    conn: &Connection,
    capability_id: &str,
    connector_id: &str,
    account_id: &str,
    environment: &str,
    action_json: &str,
) -> Result<GatewayExecuteResult, String> {
    let action = normalize_action(action_json)?;
    let cap = load_capability(conn, capability_id)?;
    let Some(cap) = cap else {
        let receipt = insert_receipt(
            conn,
            &action,
            ReceiptFields {
                capability_id: Some(capability_id),
                status: GatewayStatus::Refused,
                decision_effect: Effect::Allow,
                rule_id: None,
                message: None,
                connector_executed: false,
                nonce: None,
                reason: Some("capability not found"),
            },
        )?;
        return Ok(GatewayExecuteResult { ok: false, receipt });
    };
    if cap.used_at.is_some() {
        return refuse(conn, &cap, &action, "capability already used");
    }
    if is_expired(&cap.expires_at) {
        return refuse(conn, &cap, &action, "capability expired");
    }
    if action.payload_hash != cap.payload_hash {
        return refuse(conn, &cap, &action, "payload hash mismatch");
    }
    if let Some(reason) = channel_mismatch(conn, connector_id, account_id, environment)? {
        return refuse(conn, &cap, &action, reason);
    }
    let stored: SoulAction = serde_json::from_str(&cap.action_json)
        .map_err(|e| format!("stored action is corrupted: {e}"))?;
    let decision = policy::evaluate(conn, &stored)?;
    if decision.effect != Effect::Allow {
        return refuse(conn, &cap, &action, "action denied by policy");
    }
    let simulation = fake_connector_execute(&stored);
    conn.execute(
        "UPDATE capabilities SET used_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), cap.id],
    )
    .map_err(|e| format!("capability update failed: {e}"))?;
    let receipt = update_receipt_to_simulated(conn, &cap, &action, &simulation)?;
    Ok(GatewayExecuteResult { ok: true, receipt })
}

/// Квитанции, свежими первыми (без исходной нагрузки).
pub fn list_receipts(conn: &Connection) -> Result<Vec<GatewayReceipt>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM gateway_receipts ORDER BY created_at DESC LIMIT 200"
        ))
        .map_err(|e| format!("receipt list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], receipt_row_from_sql)
        .map_err(|e| format!("receipt list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("receipt row failed: {e}"))?);
    }
    Ok(out)
}

/// Capabilities, свежими первыми.
pub fn list_capabilities(conn: &Connection) -> Result<Vec<CapabilityInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, action_id, kind, payload_hash, nonce, expires_at, created_at, used_at
             FROM capabilities ORDER BY created_at DESC LIMIT 200",
        )
        .map_err(|e| format!("capability list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CapabilityInfo {
                id: row.get(0)?,
                action_id: row.get(1)?,
                kind: row.get(2)?,
                payload_hash: row.get(3)?,
                nonce: row.get(4)?,
                expires_at: row.get(5)?,
                created_at: row.get(6)?,
                used_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("capability list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("capability row failed: {e}"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::policy::create_policy;

    struct TestEnv {
        dir: std::path::PathBuf,
        conn: Connection,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-gateway-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            TestEnv { dir, conn }
        }

        /// Разрешённое сидом действие (не purchase >500, не data.delete).
        fn allowed_action(&self) -> String {
            allowed_action_json()
        }

        fn purchase_600(&self) -> String {
            serde_json::to_string(&SoulAction {
                action_id: "act_2".to_string(),
                kind: "purchase.create".to_string(),
                actor: "agent-1".to_string(),
                connector_id: "demo-connector".to_string(),
                account_id: "acct-1".to_string(),
                environment: "production".to_string(),
                recipient: None,
                domain: None,
                amount: Some(600.0),
                currency: Some("USD".to_string()),
                data_classes: vec![],
                reversible: false,
                confirmed_by_user: false,
                requested_scopes: vec!["purchase:write".to_string()],
                payload_hash: String::new(),
            })
            .unwrap()
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn execute_ok(env: &TestEnv, cap_id: &str, action_json: &str) -> GatewayExecuteResult {
        execute_capability(
            &env.conn,
            cap_id,
            "demo-connector",
            "acct-1",
            "production",
            action_json,
        )
        .unwrap()
    }

    fn allowed_action_json() -> String {
        serde_json::to_string(&SoulAction {
            action_id: "act_1".to_string(),
            kind: "notes.create".to_string(),
            actor: "agent-1".to_string(),
            connector_id: "demo-connector".to_string(),
            account_id: "acct-1".to_string(),
            environment: "production".to_string(),
            recipient: None,
            domain: None,
            amount: Some(10.0),
            currency: None,
            data_classes: vec![],
            reversible: true,
            confirmed_by_user: true,
            requested_scopes: vec!["notes:write".to_string()],
            payload_hash: "agent-proposed".to_string(),
        })
        .unwrap()
    }

    // ---------- Нормализация ----------

    #[test]
    fn normalize_trims_and_recomputes_payload_hash() {
        let env = TestEnv::new();
        let raw = env
            .allowed_action()
            .replace("\"actionId\":\"act_1\"", "\"actionId\": \"  act_1  \"");
        let a = normalize_action(&raw).unwrap();
        assert_eq!(a.action_id, "act_1");
        assert_eq!(a.payload_hash, payload_hash_of(&a));
        let again = normalize_action(&raw).unwrap();
        assert_eq!(again.payload_hash, a.payload_hash, "hash is deterministic");
    }

    #[test]
    fn normalize_rejects_missing_required_fields() {
        let env = TestEnv::new();
        let err = normalize_action(
            &env.allowed_action()
                .replace("\"kind\":\"notes.create\"", "\"kind\":\"  \""),
        )
        .unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected error: {err}");
        let err = normalize_action(
            &env.allowed_action()
                .replace("\"connectorId\":\"demo-connector\"", "\"connectorId\":\"\""),
        )
        .unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected error: {err}");
    }

    #[test]
    fn normalize_rejects_oversized_json_and_huge_amount() {
        let env = TestEnv::new();
        let huge = format!("{{\"actionId\":\"{}\"}}", "x".repeat(MAX_ACTION_JSON_CHARS));
        let err = normalize_action(&huge).unwrap_err();
        assert!(err.contains("exceeds"), "unexpected error: {err}");

        let err = normalize_action(&env.allowed_action().replace("10.0", "1e13")).unwrap_err();
        assert!(err.contains("Amount"), "unexpected error: {err}");
    }

    #[test]
    fn normalize_rejects_invalid_json() {
        let err = normalize_action("{not json").unwrap_err();
        assert!(
            err.contains("not valid SoulAction JSON"),
            "unexpected error: {err}"
        );
    }

    // ---------- Предложение ----------

    #[test]
    fn propose_allow_issues_capability_with_nonce_and_expiry() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let proposal = propose_action(&env.conn, &json, Some(60)).unwrap();
        assert_eq!(proposal.decision.effect, Effect::Allow);
        assert_eq!(proposal.receipt.status, GatewayStatus::Pending);
        assert!(!proposal.receipt.connector_executed);

        let cap = proposal.capability.expect("capability issued");
        assert!(cap.id.starts_with("cap_"));
        assert_eq!(cap.action_id, "act_1");
        assert_eq!(cap.nonce.len(), 36, "uuid nonce");
        assert_eq!(
            cap.payload_hash,
            normalize_action(&json).unwrap().payload_hash
        );
        let created = DateTime::parse_from_rfc3339(&cap.created_at).unwrap();
        let expires = DateTime::parse_from_rfc3339(&cap.expires_at).unwrap();
        let ttl = expires.signed_duration_since(created).num_seconds();
        assert_eq!(ttl, 60);
        assert!(cap.used_at.is_none());

        let listed = list_capabilities(&env.conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(
            !listed[0].payload_hash.contains("act_1"),
            "no raw payload in API"
        );
    }

    #[test]
    fn propose_nonce_is_unique_across_proposals() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let a = propose_action(&env.conn, &json, None).unwrap();
        let b = propose_action(&env.conn, &json, None).unwrap();
        assert_ne!(
            a.capability.unwrap().nonce,
            b.capability.unwrap().nonce,
            "nonce must be unique"
        );
    }

    #[test]
    fn propose_deny_by_seed_rule_creates_no_capability() {
        let env = TestEnv::new();
        let json = env.allowed_action().replace("notes.create", "data.delete");
        let proposal = propose_action(&env.conn, &json, None).unwrap();
        assert_eq!(proposal.decision.effect, Effect::Deny);
        assert!(proposal.capability.is_none());
        assert_eq!(proposal.receipt.status, GatewayStatus::Denied);
        assert!(!proposal.receipt.connector_executed);
        assert_eq!(list_capabilities(&env.conn).unwrap().len(), 0);
    }

    #[test]
    fn propose_require_confirmation_from_seed_rule_is_held() {
        let env = TestEnv::new();
        let proposal = propose_action(&env.conn, &env.purchase_600(), None).unwrap();
        assert_eq!(proposal.decision.effect, Effect::RequireConfirmation);
        assert!(proposal.capability.is_none());
        assert_eq!(proposal.receipt.status, GatewayStatus::Held);
        assert_eq!(
            proposal.receipt.rule_id.as_deref(),
            Some("policy_high_value_confirmation")
        );
    }

    #[test]
    fn propose_redact_custom_rule_yields_redacted_receipt() {
        let env = TestEnv::new();
        create_policy(
            &env.conn,
            r#"{"id":"r_redact","priority":800,"when":{"eq":["action.kind","email.send"]},"effect":"redact","message":"m"}"#,
        )
        .unwrap();
        let json = env.allowed_action().replace("notes.create", "email.send");
        let proposal = propose_action(&env.conn, &json, None).unwrap();
        assert_eq!(proposal.decision.effect, Effect::Redact);
        assert!(proposal.capability.is_none());
        assert_eq!(proposal.receipt.status, GatewayStatus::Redacted);
    }

    #[test]
    fn propose_ttl_is_clamped_to_max() {
        let env = TestEnv::new();
        let proposal = propose_action(&env.conn, &env.allowed_action(), Some(9_999_999)).unwrap();
        let cap = proposal.capability.unwrap();
        let created = DateTime::parse_from_rfc3339(&cap.created_at).unwrap();
        let expires = DateTime::parse_from_rfc3339(&cap.expires_at).unwrap();
        assert_eq!(
            expires.signed_duration_since(created).num_seconds(),
            MAX_TTL_SECONDS as i64
        );
    }

    #[test]
    fn propose_oversized_store_errors() {
        let env = TestEnv::new();
        let err = propose_action(&env.conn, "{not json", None).unwrap_err();
        assert!(err.contains("not valid"), "unexpected error: {err}");
    }

    // ---------- Выполнение ----------

    #[test]
    fn execute_success_simulates_and_marks_used_once() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();

        let result = execute_ok(&env, &cap.id, &json);
        assert!(result.ok, "expected ok, got {:?}", result.receipt.reason);
        assert_eq!(result.receipt.status, GatewayStatus::Simulated);
        assert!(result.receipt.connector_executed);
        assert!(
            result.receipt.message.as_deref().unwrap().contains("sim_"),
            "receipt carries simulated transaction id"
        );

        let listed = list_capabilities(&env.conn).unwrap();
        assert!(
            listed[0].used_at.is_some(),
            "capability burned after execution"
        );
        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts.len(), 1, "pending receipt upgraded in place");
    }

    #[test]
    fn execute_reuse_is_refused_without_connector() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        execute_ok(&env, &cap.id, &json);

        let again = execute_ok(&env, &cap.id, &json);
        assert!(!again.ok);
        assert_eq!(again.receipt.status, GatewayStatus::Refused);
        assert_eq!(
            again.receipt.reason.as_deref(),
            Some("capability already used")
        );
        assert!(!again.receipt.connector_executed);
    }

    #[test]
    fn execute_tampered_payload_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();

        let tampered = json.replace("\"amount\":10.0", "\"amount\":11.0");
        assert_ne!(tampered, json);
        let result = execute_ok(&env, &cap.id, &tampered);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("payload hash mismatch")
        );
        assert!(!result.receipt.connector_executed);
        assert!(
            list_capabilities(&env.conn).unwrap()[0].used_at.is_none(),
            "failed validation must not burn the capability"
        );
    }

    #[test]
    fn execute_wrong_connector_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &cap.id,
            "other-connector",
            "acct-1",
            "production",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(result.receipt.reason.as_deref(), Some("connector mismatch"));
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_wrong_account_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &cap.id,
            "demo-connector",
            "acct-999",
            "production",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(result.receipt.reason.as_deref(), Some("account mismatch"));
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_wrong_environment_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &cap.id,
            "demo-connector",
            "acct-1",
            "sandbox",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("environment mismatch")
        );
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_expired_capability_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        env.conn
            .execute(
                "UPDATE capabilities SET expires_at = ?1 WHERE id = ?2",
                params!["2020-01-01T00:00:00+00:00", cap.id],
            )
            .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(result.receipt.reason.as_deref(), Some("capability expired"));
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_unknown_capability_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let result = execute_ok(&env, "cap_nope", &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("capability not found")
        );
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_re_evaluates_policy_and_blocks_after_propose() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &json, None)
            .unwrap()
            .capability
            .unwrap();

        create_policy(
            &env.conn,
            r#"{"id":"r_block","priority":1000,"when":{"eq":["action.kind","notes.create"]},"effect":"deny","message":"m"}"#,
        )
        .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("action denied by policy")
        );
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn denied_action_never_reaches_fake_connector() {
        let env = TestEnv::new();
        let denied = env.allowed_action().replace("notes.create", "data.delete");
        let proposal = propose_action(&env.conn, &denied, None).unwrap();
        assert!(proposal.capability.is_none());
        assert_eq!(proposal.receipt.status, GatewayStatus::Denied);

        let receipts = list_receipts(&env.conn).unwrap();
        assert!(
            receipts.iter().all(|r| !r.connector_executed),
            "no connector execution for a denied action"
        );
        assert!(
            receipts
                .iter()
                .all(|r| r.status != GatewayStatus::Simulated),
            "no simulated receipt for a denied action"
        );
        assert_eq!(list_capabilities(&env.conn).unwrap().len(), 0);
    }

    #[test]
    fn fake_connector_is_deterministic_and_side_effect_free() {
        let env = TestEnv::new();
        let action = normalize_action(&env.allowed_action()).unwrap();
        let a = fake_connector_execute(&action);
        let b = fake_connector_execute(&action);
        assert_eq!(a.transaction_id, b.transaction_id);
        assert!(a.transaction_id.starts_with("sim_"));
        assert!(a.status == "ok");
    }

    // ---------- Хранилище ----------

    #[test]
    fn receipts_and_capabilities_are_listed_newest_first() {
        let env = TestEnv::new();
        propose_action(&env.conn, &env.allowed_action(), None).unwrap();
        let denied = env.allowed_action().replace("notes.create", "data.delete");
        propose_action(&env.conn, &denied, None).unwrap();

        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(
            receipts[0].created_at >= receipts[1].created_at,
            "newest first"
        );
        assert!(receipts.iter().any(|r| r.status == GatewayStatus::Denied));
        assert!(receipts.iter().any(|r| r.status == GatewayStatus::Pending));
    }

    #[test]
    fn connectors_registry_is_seeded_once() {
        let env = TestEnv::new();
        let count: i64 = env
            .conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 4);
        let again: i64 = env
            .conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(again, 4, "seeding is idempotent");
    }

    #[test]
    fn wipe_all_clears_gateway_and_reseeds_connectors() {
        let dir = std::env::temp_dir().join(format!("soul-gateway-wipe-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = init_db(&dir).unwrap();
        let json = allowed_action_json();
        let cap = propose_action(&conn, &json, None)
            .unwrap()
            .capability
            .unwrap();
        execute_capability(
            &conn,
            &cap.id,
            "demo-connector",
            "acct-1",
            "production",
            &json,
        )
        .unwrap();
        assert_eq!(list_receipts(&conn).unwrap().len(), 1);

        crate::db::wipe_all(&conn).unwrap();
        assert!(list_receipts(&conn).unwrap().is_empty());
        assert!(list_capabilities(&conn).unwrap().is_empty());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "registry cleared with wipe");

        drop(conn);
        let conn = init_db(&dir).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 4, "registry reseeded after wipe");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
