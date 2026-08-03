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
//! Review-pass (SESSION-12): capability привязывает канал (коннектор/учётная
//! запись/окружение) из действия при выдаче; capability и квитанции подписаны
//! локальным ключом устройства (ed25519, §4.11 «подписанная локальная
//! квитанция»); выполнение атомарно (used_at + квитанция в одной транзакции);
//! `require_confirmation` имеет продолжение — capability удерживается до явного
//! подтверждения пользователем; `redact` имеет продолжение — поддельный
//! коннектор получает отредактированную копию действия, чувствительные поля
//! скрыты; реестр имитированных коннекторов управляется из интерфейса
//! (добавление/удаление), оставаясь локальной имитацией.
//!
//! Ultra-review (SESSION-12): сохранённая нагрузка не доверяется — при
//! выполнении `action_json` повторно хешируется и сверяется с подписанным
//! `payload_hash`, `redacted_json` — с каноническим отредактированным вариантом
//! (вмешательство в эти колонки не покрывается подписью и ловится отдельно,
//! fail-closed); `propose_action` атомарен (capability + квитанция в одной
//! транзакции); `environment` — обязательное поле наравне с остальными.
//!
//! Каждый шаг оставляет квитанцию со статусом имитации: pending (capability
//! выдана), simulated (поддельный коннектор выполнил действие), denied
//! (запрещено политикой), held (ожидает подтверждения пользователя), redacted
//! (данные скрыты политикой), refused (отказ на этапе выполнения: повтор,
//! изменённая нагрузка, неверный канал, истёкший срок, политика на момент
//! выполнения). Квитанции не содержат исходной чувствительной нагрузки —
//! только hash и метаданные действия.
//!
//! Канал выполнения проверяется по локальному реестру имитированных
//! коннекторов (`gateway_connectors`), который сеется один раз за жизнь
//! хранилища (как демо-политики SESSION-11) и может управляться пользователем.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::Serialize;
use uuid::Uuid;

use crate::crypto::{self, DeviceKeys};
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
/// Максимальное число каналов в реестре имитированных коннекторов.
pub const MAX_GATEWAY_CONNECTORS: usize = 50;
/// Максимальная длина одного поля канала.
pub const MAX_CHANNEL_FIELD_CHARS: usize = 64;

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
    /// Политика скрыла действие (данные отредактированы).
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
    /// Канал, к которому capability привязана при выдаче.
    pub connector_id: String,
    pub account_id: String,
    pub environment: String,
    /// Эффект решения при выдаче: allow / require_confirmation / redact.
    pub decision_effect: String,
    /// Подтверждена ли capability пользователем (для require_confirmation).
    pub confirmed_by_user: bool,
    /// Действие выполнялось бы с отредактированной нагрузкой (redact).
    pub redacted: bool,
    /// Подпись локального устройства (base64, ed25519).
    pub signature: String,
    pub signer_public_key: String,
    /// Подпись проверена по сохранённому публичному ключу.
    pub signature_valid: bool,
}

/// Строка capabilities вместе с сохранённой нагрузкой (для повторной оценки
/// политики в момент выполнения) и подписью. Нагрузка не выходит наружу.
struct CapabilityRow {
    id: String,
    action_id: String,
    kind: String,
    payload_hash: String,
    nonce: String,
    action_json: String,
    redacted_json: Option<String>,
    expires_at: String,
    created_at: String,
    used_at: Option<String>,
    connector_id: String,
    account_id: String,
    environment: String,
    decision_effect: Effect,
    confirmed_by_user: bool,
    signature: String,
    signer_public_key: String,
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
    /// Подпись локального устройства (base64, ed25519).
    pub signature: String,
    pub signer_public_key: String,
    /// Подпись проверена по сохранённому публичному ключу.
    pub signature_valid: bool,
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

/// Канал в локальном реестре имитированных коннекторов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayChannel {
    pub connector_id: String,
    pub account_id: String,
    pub environment: String,
}

// ---------- Подпись локальным устройством (ed25519) ----------

fn sign(keys: &DeviceKeys, message: &str) -> String {
    B64.encode(crypto::sign_bytes(&keys.private_bytes, message.as_bytes()))
}

fn verify(public_b64: &str, message: &str, signature_b64: &str) -> bool {
    let Ok(sig) = B64.decode(signature_b64) else {
        return false;
    };
    crypto::verify_signature(public_b64, message.as_bytes(), &sig)
}

/// Каноническое подписываемое сообщение capability: все неизменяемые поля,
/// включая привязанный канал, эффект решения и состояние подтверждения.
#[allow(clippy::too_many_arguments)] // поля подписи = неизменяемые поля строки
fn capability_signing_message(
    id: &str,
    action_id: &str,
    payload_hash: &str,
    nonce: &str,
    expires_at: &str,
    created_at: &str,
    connector_id: &str,
    account_id: &str,
    environment: &str,
    decision_effect: &str,
    confirmed_by_user: bool,
) -> String {
    format!(
        "soul-capability/1|{id}|{action_id}|{payload_hash}|{nonce}|{expires_at}|{created_at}|{connector_id}|{account_id}|{environment}|{decision_effect}|{confirmed_by_user}"
    )
}

/// Каноническое подписываемое сообщение квитанции (None → пустая строка,
/// детерминированно).
#[allow(clippy::too_many_arguments)]
fn receipt_signing_message(
    id: &str,
    capability_id: &str,
    action_id: &str,
    kind: &str,
    status: &str,
    decision_effect: &str,
    rule_id: &str,
    message: &str,
    connector_executed: bool,
    reason: &str,
    nonce: &str,
    created_at: &str,
) -> String {
    format!(
        "soul-gateway-receipt/1|{id}|{capability_id}|{action_id}|{kind}|{status}|{decision_effect}|{rule_id}|{message}|{connector_executed}|{reason}|{nonce}|{created_at}"
    )
}

fn sign_capability(keys: &DeviceKeys, cap: &CapabilityRow) -> String {
    sign(
        keys,
        &capability_signing_message(
            &cap.id,
            &cap.action_id,
            &cap.payload_hash,
            &cap.nonce,
            &cap.expires_at,
            &cap.created_at,
            &cap.connector_id,
            &cap.account_id,
            &cap.environment,
            effect_to_str(cap.decision_effect),
            cap.confirmed_by_user,
        ),
    )
}

fn capability_signature_valid(cap: &CapabilityRow) -> bool {
    verify(
        &cap.signer_public_key,
        &capability_signing_message(
            &cap.id,
            &cap.action_id,
            &cap.payload_hash,
            &cap.nonce,
            &cap.expires_at,
            &cap.created_at,
            &cap.connector_id,
            &cap.account_id,
            &cap.environment,
            effect_to_str(cap.decision_effect),
            cap.confirmed_by_user,
        ),
        &cap.signature,
    )
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
        || action.environment.is_empty()
    {
        return Err(
            "Required action fields (actionId, kind, actor, connectorId, accountId, environment) must not be empty."
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

/// Отредактированная копия действия для эффекта `redact`: структурные поля
/// сохраняются (actionId/kind/actor/канал/reversible/confirmedByUser),
/// чувствительные данные (получатель, домен, сумма, валюта, классы данных,
/// запрашиваемые области) скрываются. Поддельный коннектор получает именно
/// этот вариант — чувствительная нагрузка не «выполняется».
fn redact_variant(action: &SoulAction) -> SoulAction {
    let mut a = action.clone();
    a.recipient = None;
    a.domain = None;
    a.amount = None;
    a.currency = None;
    a.data_classes = Vec::new();
    a.requested_scopes = Vec::new();
    a.payload_hash = String::new();
    a
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

/// Добавление отсутствующей колонки в существующую таблицу (миграция БД из
/// первой версии SESSION-12). Имена таблиц/колонок — константы, не пользователь.
fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> SqlResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !existing.iter().any(|c| c == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
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
            used_at TEXT,
            connector_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            environment TEXT NOT NULL,
            decision_effect TEXT NOT NULL,
            confirmed_by_user INTEGER NOT NULL DEFAULT 0,
            redacted_json TEXT,
            signature TEXT,
            signer_public_key TEXT
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
            created_at TEXT NOT NULL,
            signature TEXT,
            signer_public_key TEXT
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
    for (column, decl) in [
        ("connector_id", "TEXT NOT NULL DEFAULT ''"),
        ("account_id", "TEXT NOT NULL DEFAULT ''"),
        ("environment", "TEXT NOT NULL DEFAULT ''"),
        ("decision_effect", "TEXT NOT NULL DEFAULT 'allow'"),
        ("confirmed_by_user", "INTEGER NOT NULL DEFAULT 0"),
        ("redacted_json", "TEXT"),
        ("signature", "TEXT"),
        ("signer_public_key", "TEXT"),
    ] {
        ensure_column(conn, "capabilities", column, decl)?;
    }
    for (column, decl) in [("signature", "TEXT"), ("signer_public_key", "TEXT")] {
        ensure_column(conn, "gateway_receipts", column, decl)?;
    }
    seed_connectors(conn)
}

/// Реестр имитированных коннекторов — один раз за жизнь хранилища (флаг в
/// `gateway_meta`), как демо-политики SESSION-11. Канал выполнения должен
/// присутствовать в реестре, иначе выполнение отказывается. Реестр можно
/// пополнять и очищать из интерфейса (`add_connector` / `remove_connector`).
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

fn receipt_signing_message_from_fields(receipt: &GatewayReceipt) -> String {
    receipt_signing_message(
        &receipt.id,
        receipt.capability_id.as_deref().unwrap_or(""),
        &receipt.action_id,
        &receipt.kind,
        receipt.status.as_str(),
        effect_to_str(receipt.decision_effect),
        receipt.rule_id.as_deref().unwrap_or(""),
        receipt.message.as_deref().unwrap_or(""),
        receipt.connector_executed,
        receipt.reason.as_deref().unwrap_or(""),
        receipt.nonce.as_deref().unwrap_or(""),
        &receipt.created_at,
    )
}

fn insert_receipt(
    conn: &Connection,
    keys: &DeviceKeys,
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
        signature: String::new(),
        signer_public_key: keys.public_b64.clone(),
        signature_valid: true,
    };
    let signature = sign(keys, &receipt_signing_message_from_fields(&receipt));
    let mut receipt = receipt;
    receipt.signature = signature;
    conn.execute(
        "INSERT INTO gateway_receipts (
            id, capability_id, action_id, kind, status, decision_effect,
            rule_id, message, connector_executed, reason, nonce, created_at,
            signature, signer_public_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
            receipt.created_at,
            receipt.signature,
            receipt.signer_public_key
        ],
    )
    .map_err(|e| format!("gateway receipt insert failed: {e}"))?;
    Ok(receipt)
}

/// Этап предложения: нормализация → оценка политикой → capability/квитанция.
/// Capability привязывается к каналу из действия и подписывается локальным
/// устройством. Для `require_confirmation` capability удерживается до
/// `confirm_capability`; для `redact` поддельный коннектор получит
/// отредактированную копию действия.
pub fn propose_action(
    conn: &Connection,
    keys: &DeviceKeys,
    action_json: &str,
    ttl_seconds: Option<u64>,
) -> Result<GatewayProposal, String> {
    let action = normalize_action(action_json)?;
    let decision = policy::evaluate(conn, &action)?;
    if decision.effect == Effect::Deny {
        let receipt = insert_receipt(
            conn,
            keys,
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

    // Capability выдаётся для allow / require_confirmation / redact; канал
    // привязывается из действия и обязан быть в локальном реестре.
    if channel_mismatch(
        conn,
        &action.connector_id,
        &action.account_id,
        &action.environment,
    )?
    .is_some()
    {
        return Err(format!(
            "Channel ({}, {}, {}) is not in the simulated connector registry.",
            action.connector_id, action.account_id, action.environment
        ));
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
    let confirmed_by_user = decision.effect != Effect::RequireConfirmation;
    let redacted_json = if decision.effect == Effect::Redact {
        Some(canonical_action_json(&redact_variant(&action)))
    } else {
        None
    };
    let cap_row = CapabilityRow {
        id: id.clone(),
        action_id: action.action_id.clone(),
        kind: action.kind.clone(),
        payload_hash: action.payload_hash.clone(),
        nonce: nonce.clone(),
        action_json: canonical_action_json(&action),
        redacted_json,
        expires_at: expires_at.clone(),
        created_at: now.to_rfc3339(),
        used_at: None,
        connector_id: action.connector_id.clone(),
        account_id: action.account_id.clone(),
        environment: action.environment.clone(),
        decision_effect: decision.effect,
        confirmed_by_user,
        signature: String::new(),
        signer_public_key: keys.public_b64.clone(),
    };
    let signature = sign_capability(keys, &cap_row);
    // Capability и квитанция в одной транзакции: сбой вставки квитанции
    // (лимит списка) откатывает capability — никаких осиротевших строк.
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("propose transaction failed: {e}"))?;
    tx.execute(
        "INSERT INTO capabilities (
            id, action_id, kind, payload_hash, nonce, action_json, redacted_json,
            expires_at, created_at, connector_id, account_id, environment,
            decision_effect, confirmed_by_user, signature, signer_public_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            id,
            action.action_id,
            action.kind,
            action.payload_hash,
            nonce,
            cap_row.action_json,
            cap_row.redacted_json,
            expires_at,
            cap_row.created_at,
            cap_row.connector_id,
            cap_row.account_id,
            cap_row.environment,
            effect_to_str(cap_row.decision_effect),
            if cap_row.confirmed_by_user { 1 } else { 0 },
            signature,
            cap_row.signer_public_key
        ],
    )
    .map_err(|e| format!("capability insert failed: {e}"))?;
    let mut cap_row = cap_row;
    cap_row.signature = signature;
    let capability = capability_info_from_row(&cap_row);
    let receipt = insert_receipt(
        &tx,
        keys,
        &action,
        ReceiptFields {
            capability_id: Some(&id),
            status: status_for(decision.effect),
            decision_effect: decision.effect,
            rule_id: decision.rule_id.as_deref(),
            message: decision.message.as_deref(),
            connector_executed: false,
            nonce: Some(&nonce),
            reason: None,
        },
    )?;
    tx.commit()
        .map_err(|e| format!("propose commit failed: {e}"))?;
    Ok(GatewayProposal {
        decision,
        capability: Some(capability),
        receipt,
    })
}

fn capability_info_from_row(cap: &CapabilityRow) -> CapabilityInfo {
    CapabilityInfo {
        id: cap.id.clone(),
        action_id: cap.action_id.clone(),
        kind: cap.kind.clone(),
        payload_hash: cap.payload_hash.clone(),
        nonce: cap.nonce.clone(),
        expires_at: cap.expires_at.clone(),
        created_at: cap.created_at.clone(),
        used_at: cap.used_at.clone(),
        connector_id: cap.connector_id.clone(),
        account_id: cap.account_id.clone(),
        environment: cap.environment.clone(),
        decision_effect: effect_to_str(cap.decision_effect).to_string(),
        confirmed_by_user: cap.confirmed_by_user,
        redacted: cap.decision_effect == Effect::Redact,
        signature: cap.signature.clone(),
        signer_public_key: cap.signer_public_key.clone(),
        signature_valid: capability_signature_valid(cap),
    }
}

fn capability_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<CapabilityRow> {
    Ok(CapabilityRow {
        id: row.get(0)?,
        action_id: row.get(1)?,
        kind: row.get(2)?,
        payload_hash: row.get(3)?,
        nonce: row.get(4)?,
        action_json: row.get(5)?,
        redacted_json: row.get(6)?,
        expires_at: row.get(7)?,
        created_at: row.get(8)?,
        used_at: row.get(9)?,
        connector_id: row.get(10)?,
        account_id: row.get(11)?,
        environment: row.get(12)?,
        decision_effect: effect_from_str(&row.get::<_, String>(13)?),
        confirmed_by_user: row.get::<_, i64>(14)? != 0,
        signature: row.get(15)?,
        signer_public_key: row.get(16)?,
    })
}

const CAPABILITY_COLUMNS: &str =
    "id, action_id, kind, payload_hash, nonce, action_json, redacted_json, expires_at, \
     created_at, used_at, connector_id, account_id, environment, decision_effect, \
     confirmed_by_user, signature, signer_public_key";

fn load_capability(
    conn: &Connection,
    capability_id: &str,
) -> Result<Option<CapabilityRow>, String> {
    conn.query_row(
        &format!("SELECT {CAPABILITY_COLUMNS} FROM capabilities WHERE id = ?1"),
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
    keys: &DeviceKeys,
    cap: &CapabilityRow,
    action: &SoulAction,
    reason: &'static str,
) -> Result<GatewayExecuteResult, String> {
    let receipt = insert_receipt(
        conn,
        keys,
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
    let receipt = GatewayReceipt {
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
        signature: row.get(12)?,
        signer_public_key: row.get(13)?,
        signature_valid: false,
    };
    let signature_valid = verify(
        &receipt.signer_public_key,
        &receipt_signing_message_from_fields(&receipt),
        &receipt.signature,
    );
    Ok(GatewayReceipt {
        signature_valid,
        ..receipt
    })
}

const RECEIPT_COLUMNS: &str =
    "id, capability_id, action_id, kind, status, decision_effect, rule_id, message, connector_executed, reason, nonce, created_at, signature, signer_public_key";

fn update_receipt_to_simulated(
    conn: &Connection,
    keys: &DeviceKeys,
    cap: &CapabilityRow,
    action: &SoulAction,
    simulation: &ConnectorSimulation,
) -> Result<GatewayReceipt, String> {
    let redacted = cap.decision_effect == Effect::Redact;
    let message = if redacted {
        format!(
            "simulated transaction {} — payload redacted; no data exposed",
            simulation.transaction_id
        )
    } else {
        format!(
            "simulated transaction {} — {}",
            simulation.transaction_id, simulation.note
        )
    };
    let pending: Option<GatewayReceipt> = conn
        .query_row(
            &format!(
                "SELECT {RECEIPT_COLUMNS} FROM gateway_receipts
                 WHERE capability_id = ?1 AND status IN ('pending','held','redacted')
                 ORDER BY created_at DESC LIMIT 1"
            ),
            params![cap.id],
            receipt_row_from_sql,
        )
        .optional()
        .map_err(|e| format!("receipt lookup failed: {e}"))?;
    if let Some(mut receipt) = pending {
        receipt.status = GatewayStatus::Simulated;
        receipt.connector_executed = true;
        receipt.message = Some(message);
        receipt.signature = String::new();
        receipt.signature_valid = true;
        let signature = sign(keys, &receipt_signing_message_from_fields(&receipt));
        receipt.signature = signature;
        conn.execute(
            "UPDATE gateway_receipts
             SET status = 'simulated', connector_executed = 1, message = ?1,
                 signature = ?2, signer_public_key = ?3
             WHERE id = ?4",
            params![
                receipt.message,
                receipt.signature,
                keys.public_b64,
                receipt.id
            ],
        )
        .map_err(|e| format!("receipt update failed: {e}"))?;
        return Ok(receipt);
    }
    insert_receipt(
        conn,
        keys,
        action,
        ReceiptFields {
            capability_id: Some(&cap.id),
            status: GatewayStatus::Simulated,
            decision_effect: cap.decision_effect,
            rule_id: None,
            message: Some(&message),
            connector_executed: true,
            nonce: Some(&cap.nonce),
            reason: None,
        },
    )
}

/// Подтверждение capability пользователем (для `require_confirmation`):
/// квитанция held → pending, capability становится выполнимой. P0-имитация
/// локального потока подтверждения — никакого реального внешнего вызова.
pub fn confirm_capability(
    conn: &Connection,
    keys: &DeviceKeys,
    capability_id: &str,
) -> Result<CapabilityInfo, String> {
    let cap =
        load_capability(conn, capability_id)?.ok_or_else(|| "Capability not found.".to_string())?;
    if !capability_signature_valid(&cap) {
        return Err("Invalid capability signature.".to_string());
    }
    if cap.used_at.is_some() {
        return Err("Capability already used.".to_string());
    }
    if is_expired(&cap.expires_at) {
        return Err("Capability expired.".to_string());
    }
    if cap.decision_effect != Effect::RequireConfirmation {
        return Err("Capability does not require confirmation.".to_string());
    }
    if cap.confirmed_by_user {
        return Err("Capability already confirmed.".to_string());
    }

    let mut cap = cap;
    cap.confirmed_by_user = true;
    // Подпись покрывает состояние подтверждения: после изменения — переподпись.
    let signature = sign_capability(keys, &cap);
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("confirm transaction failed: {e}"))?;
    tx.execute(
        "UPDATE capabilities SET confirmed_by_user = 1, signature = ?1, signer_public_key = ?2
         WHERE id = ?3",
        params![signature, keys.public_b64, cap.id],
    )
    .map_err(|e| format!("capability confirm failed: {e}"))?;
    cap.signature = signature;
    if let Some(mut receipt) = tx
        .query_row(
            &format!(
                "SELECT {RECEIPT_COLUMNS} FROM gateway_receipts
                 WHERE capability_id = ?1 AND status = 'held' ORDER BY created_at DESC LIMIT 1"
            ),
            params![cap.id],
            receipt_row_from_sql,
        )
        .optional()
        .map_err(|e| format!("receipt lookup failed: {e}"))?
    {
        receipt.status = GatewayStatus::Pending;
        receipt.message = Some("confirmed by user".to_string());
        receipt.signature = String::new();
        let signature = sign(keys, &receipt_signing_message_from_fields(&receipt));
        receipt.signature = signature;
        tx.execute(
            "UPDATE gateway_receipts
             SET status = 'pending', message = ?1, signature = ?2, signer_public_key = ?3
             WHERE id = ?4",
            params![
                receipt.message,
                receipt.signature,
                keys.public_b64,
                receipt.id
            ],
        )
        .map_err(|e| format!("receipt update failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("confirm commit failed: {e}"))?;

    Ok(capability_info_from_row(&cap))
}

/// Этап выполнения: capability → подпись → привязанный канал → повторная оценка
/// политики → поддельный коннектор. Любой отказ оставляет квитанцию `refused`
/// без обращения к коннектору; успех атомарно (одна транзакция) помечает
/// capability использованной и обновляет квитанцию до `simulated`.
pub fn execute_capability(
    conn: &Connection,
    keys: &DeviceKeys,
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
            keys,
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
    if !capability_signature_valid(&cap) {
        return refuse(conn, keys, &cap, &action, "invalid capability signature");
    }
    if cap.used_at.is_some() {
        return refuse(conn, keys, &cap, &action, "capability already used");
    }
    if is_expired(&cap.expires_at) {
        return refuse(conn, keys, &cap, &action, "capability expired");
    }
    if connector_id != cap.connector_id
        || account_id != cap.account_id
        || environment != cap.environment
    {
        return refuse(
            conn,
            keys,
            &cap,
            &action,
            "capability bound to different channel",
        );
    }
    if let Some(reason) = channel_mismatch(conn, connector_id, account_id, environment)? {
        return refuse(conn, keys, &cap, &action, reason);
    }
    if action.payload_hash != cap.payload_hash {
        return refuse(conn, keys, &cap, &action, "payload hash mismatch");
    }
    let stored: SoulAction = serde_json::from_str(&cap.action_json)
        .map_err(|e| format!("stored action is corrupted: {e}"))?;
    // Подпись покрывает hash нагрузки, а не само сохранённое действие: локальное
    // вмешательство в `action_json` (или `redacted_json`) сверяется здесь с
    // подписанными ожиданиями — fail-closed, как и подделка остальных полей.
    if payload_hash_of(&stored) != cap.payload_hash {
        return refuse(conn, keys, &cap, &action, "stored action tampered");
    }
    if let Some(redacted_json) = &cap.redacted_json {
        let expected = canonical_action_json(&redact_variant(&stored));
        if redacted_json != &expected {
            return refuse(conn, keys, &cap, &action, "stored action tampered");
        }
    }
    let decision = policy::evaluate(conn, &stored)?;
    // Жёсткий блок — только Deny: для held/redact-capability повторная оценка
    // на момент выполнения закономерно возвращает require_confirmation/redact
    // (продолжение потока уже согласовано на этапе выдачи), а не отказ.
    if decision.effect == Effect::Deny {
        return refuse(conn, keys, &cap, &action, "action denied by policy");
    }
    if cap.decision_effect == Effect::RequireConfirmation && !cap.confirmed_by_user {
        return refuse(conn, keys, &cap, &action, "confirmation required");
    }
    let exec_action = match &cap.redacted_json {
        Some(redacted) => serde_json::from_str(redacted)
            .map_err(|e| format!("redacted action is corrupted: {e}"))?,
        None => stored,
    };
    let simulation = fake_connector_execute(&exec_action);

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("execute transaction failed: {e}"))?;
    tx.execute(
        "UPDATE capabilities SET used_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), cap.id],
    )
    .map_err(|e| format!("capability update failed: {e}"))?;
    let receipt = update_receipt_to_simulated(&tx, keys, &cap, &action, &simulation)?;
    tx.commit()
        .map_err(|e| format!("execute commit failed: {e}"))?;
    Ok(GatewayExecuteResult { ok: true, receipt })
}

/// Квитанции, свежими первыми (без исходной нагрузки). Подпись каждой
/// квитанции проверяется: подделанные строки помечаются `signature_valid =
/// false` (честный аудит-след).
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

/// Capabilities, свежими первыми. Подпись каждой capability проверяется.
pub fn list_capabilities(conn: &Connection) -> Result<Vec<CapabilityInfo>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {CAPABILITY_COLUMNS} FROM capabilities ORDER BY created_at DESC LIMIT 200"
        ))
        .map_err(|e| format!("capability list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], capability_row_from_sql)
        .map_err(|e| format!("capability list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let row = row.map_err(|e| format!("capability row failed: {e}"))?;
        out.push(capability_info_from_row(&row));
    }
    Ok(out)
}

// ---------- Реестр имитированных коннекторов (управление из интерфейса) ----------

fn normalize_channel(
    connector_id: &str,
    account_id: &str,
    environment: &str,
) -> Result<GatewayChannel, String> {
    let channel = GatewayChannel {
        connector_id: connector_id.trim().to_string(),
        account_id: account_id.trim().to_string(),
        environment: environment.trim().to_string(),
    };
    if channel.connector_id.is_empty()
        || channel.account_id.is_empty()
        || channel.environment.is_empty()
    {
        return Err(
            "Channel fields (connectorId, accountId, environment) must not be empty.".into(),
        );
    }
    for (name, value) in [
        ("connectorId", &channel.connector_id),
        ("accountId", &channel.account_id),
        ("environment", &channel.environment),
    ] {
        if value.chars().count() > MAX_CHANNEL_FIELD_CHARS {
            return Err(format!(
                "Channel field {name} exceeds {MAX_CHANNEL_FIELD_CHARS} characters."
            ));
        }
    }
    Ok(channel)
}

/// Каналы реестра, по возрастанию полей.
pub fn list_connectors(conn: &Connection) -> Result<Vec<GatewayChannel>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT connector_id, account_id, environment FROM gateway_connectors
             ORDER BY connector_id, account_id, environment",
        )
        .map_err(|e| format!("connector list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(GatewayChannel {
                connector_id: row.get(0)?,
                account_id: row.get(1)?,
                environment: row.get(2)?,
            })
        })
        .map_err(|e| format!("connector list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("connector row failed: {e}"))?);
    }
    Ok(out)
}

/// Добавление канала в реестр (идемпотентно).
pub fn add_connector(
    conn: &Connection,
    connector_id: &str,
    account_id: &str,
    environment: &str,
) -> Result<GatewayChannel, String> {
    let channel = normalize_channel(connector_id, account_id, environment)?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM gateway_connectors
             WHERE connector_id = ?1 AND account_id = ?2 AND environment = ?3",
            params![
                channel.connector_id,
                channel.account_id,
                channel.environment
            ],
            |r| r.get(0),
        )
        .map_err(|e| format!("connector registry query failed: {e}"))?;
    if exists == 0 {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .map_err(|e| format!("connector count failed: {e}"))?;
        if count >= MAX_GATEWAY_CONNECTORS as i64 {
            return Err(format!(
                "Too many connector channels (limit {MAX_GATEWAY_CONNECTORS})."
            ));
        }
        conn.execute(
            "INSERT OR IGNORE INTO gateway_connectors (connector_id, account_id, environment)
             VALUES (?1, ?2, ?3)",
            params![
                channel.connector_id,
                channel.account_id,
                channel.environment
            ],
        )
        .map_err(|e| format!("connector insert failed: {e}"))?;
    }
    Ok(channel)
}

/// Удаление канала из реестра. Возвращает true, если канал был удалён.
pub fn remove_connector(
    conn: &Connection,
    connector_id: &str,
    account_id: &str,
    environment: &str,
) -> Result<bool, String> {
    let changed = conn
        .execute(
            "DELETE FROM gateway_connectors
             WHERE connector_id = ?1 AND account_id = ?2 AND environment = ?3",
            params![connector_id.trim(), account_id.trim(), environment.trim()],
        )
        .map_err(|e| format!("connector delete failed: {e}"))?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::policy::create_policy;

    struct TestEnv {
        dir: std::path::PathBuf,
        conn: Connection,
        keys: DeviceKeys,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-gateway-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            let keys = crypto::ensure_device_keypair(&dir).unwrap();
            TestEnv { dir, conn, keys }
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

    fn propose(env: &TestEnv, json: &str) -> GatewayProposal {
        propose_action(&env.conn, &env.keys, json, None).unwrap()
    }

    fn execute_ok(env: &TestEnv, cap_id: &str, action_json: &str) -> GatewayExecuteResult {
        execute_capability(
            &env.conn,
            &env.keys,
            cap_id,
            "demo-connector",
            "acct-1",
            "production",
            action_json,
        )
        .unwrap()
    }

    /// Изменение срока с переподписью — так целостность (подпись) сохраняется,
    /// а проверяется именно истечение, а не защита от подделки.
    fn set_expiry(env: &TestEnv, cap_id: &str, expires_at: &str) {
        let mut cap = load_capability(&env.conn, cap_id).unwrap().unwrap();
        cap.expires_at = expires_at.to_string();
        let signature = sign_capability(&env.keys, &cap);
        env.conn
            .execute(
                "UPDATE capabilities SET expires_at = ?1, signature = ?2 WHERE id = ?3",
                params![expires_at, signature, cap_id],
            )
            .unwrap();
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
        let err = normalize_action(
            &env.allowed_action()
                .replace("\"environment\":\"production\"", "\"environment\":\"\""),
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
        let proposal = propose_action(&env.conn, &env.keys, &json, Some(60)).unwrap();
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
        assert_eq!(cap.decision_effect, "allow");
        assert!(cap.confirmed_by_user, "allow needs no confirmation");
        assert!(!cap.redacted);
        assert!(cap.signature_valid, "capability is signed by device");
        assert!(!cap.signature.is_empty());
        assert_eq!(
            cap.signer_public_key, env.keys.public_b64,
            "signed by the local device key"
        );
        assert_eq!(cap.connector_id, "demo-connector", "channel bound at issue");
        assert_eq!(cap.account_id, "acct-1");
        assert_eq!(cap.environment, "production");

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
        let a = propose_action(&env.conn, &env.keys, &json, None).unwrap();
        let b = propose_action(&env.conn, &env.keys, &json, None).unwrap();
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
        let proposal = propose_action(&env.conn, &env.keys, &json, None).unwrap();
        assert_eq!(proposal.decision.effect, Effect::Deny);
        assert!(proposal.capability.is_none());
        assert_eq!(proposal.receipt.status, GatewayStatus::Denied);
        assert!(!proposal.receipt.connector_executed);
        assert_eq!(list_capabilities(&env.conn).unwrap().len(), 0);
    }

    #[test]
    fn propose_require_confirmation_issues_held_capability() {
        let env = TestEnv::new();
        let proposal = propose(&env, &env.purchase_600());
        assert_eq!(proposal.decision.effect, Effect::RequireConfirmation);
        let cap = proposal.capability.expect("capability issued but held");
        assert_eq!(cap.decision_effect, "require_confirmation");
        assert!(!cap.confirmed_by_user, "held until user confirms");
        assert!(cap.signature_valid);
        assert_eq!(proposal.receipt.status, GatewayStatus::Held);
        assert_eq!(
            proposal.receipt.rule_id.as_deref(),
            Some("policy_high_value_confirmation")
        );
    }

    #[test]
    fn propose_redact_issues_capability_with_redacted_payload() {
        let env = TestEnv::new();
        create_policy(
            &env.conn,
            r#"{"id":"r_redact","priority":800,"when":{"eq":["action.kind","email.send"]},"effect":"redact","message":"m"}"#,
        )
        .unwrap();
        let json = env.allowed_action().replace("notes.create", "email.send");
        let proposal = propose(&env, &json);
        assert_eq!(proposal.decision.effect, Effect::Redact);
        let cap = proposal.capability.expect("capability issued for redact");
        assert!(cap.redacted);
        assert_eq!(cap.decision_effect, "redact");
        assert!(cap.signature_valid);
        assert_eq!(proposal.receipt.status, GatewayStatus::Redacted);
    }

    #[test]
    fn propose_refuses_channel_not_in_registry() {
        let env = TestEnv::new();
        let json = env
            .allowed_action()
            .replace("demo-connector", "ghost-connector");
        let err = propose_action(&env.conn, &env.keys, &json, None).unwrap_err();
        assert!(
            err.contains("not in the simulated connector registry"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn propose_ttl_is_clamped_to_max() {
        let env = TestEnv::new();
        let proposal =
            propose_action(&env.conn, &env.keys, &env.allowed_action(), Some(9_999_999)).unwrap();
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
        let err = propose_action(&env.conn, &env.keys, "{not json", None).unwrap_err();
        assert!(err.contains("not valid"), "unexpected error: {err}");
    }

    // ---------- Подпись и целостность ----------

    #[test]
    fn tampered_capability_signature_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        env.conn
            .execute(
                "UPDATE capabilities SET expires_at = ?1 WHERE id = ?2",
                params!["2020-01-01T00:00:00+00:00", cap.id],
            )
            .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("invalid capability signature")
        );
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn tampered_capability_channel_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        env.conn
            .execute(
                "UPDATE capabilities SET connector_id = 'evil' WHERE id = ?1",
                params![cap.id],
            )
            .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("invalid capability signature")
        );
    }

    #[test]
    fn tampered_receipt_is_flagged_signature_invalid() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        execute_ok(&env, &cap.id, &json);
        assert!(
            list_receipts(&env.conn).unwrap()[0].signature_valid,
            "fresh receipt signature is valid"
        );
        env.conn
            .execute("UPDATE gateway_receipts SET message = 'tampered'", [])
            .unwrap();
        let receipts = list_receipts(&env.conn).unwrap();
        assert!(
            receipts.iter().any(|r| !r.signature_valid),
            "tampered receipt must be flagged"
        );
    }

    #[test]
    fn tampered_stored_payload_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        // Подпись покрывает hash нагрузки, а не само сохранённое действие:
        // вмешательство в `action_json` ловится повторным хешированием при
        // выполнении (fail-closed), а не подписью.
        let mut forged: SoulAction = serde_json::from_str(&json).unwrap();
        forged.amount = Some(9000.0);
        env.conn
            .execute(
                "UPDATE capabilities SET action_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&forged).unwrap(), cap.id],
            )
            .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("stored action tampered")
        );
        assert!(!result.receipt.connector_executed);
        assert!(
            list_capabilities(&env.conn).unwrap()[0].signature_valid,
            "signature itself stays valid — the tamper is caught by payload re-hash"
        );
    }

    #[test]
    fn tampered_redacted_variant_is_refused() {
        let env = TestEnv::new();
        create_policy(
            &env.conn,
            r#"{"id":"r_redact","priority":800,"when":{"eq":["action.kind","email.send"]},"effect":"redact","message":"m"}"#,
        )
        .unwrap();
        let json = env.allowed_action().replace("notes.create", "email.send");
        let cap = propose(&env, &json).capability.unwrap();
        assert!(cap.redacted);
        // Вместо отредактированного варианта подложена полная нагрузка:
        // коннектор не должен «выполнить» неотредактированные данные.
        let full: SoulAction = serde_json::from_str(&json).unwrap();
        env.conn
            .execute(
                "UPDATE capabilities SET redacted_json = ?1 WHERE id = ?2",
                params![canonical_action_json(&full), cap.id],
            )
            .unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("stored action tampered")
        );
        assert!(!result.receipt.connector_executed);
    }

    // ---------- Выполнение ----------

    #[test]
    fn execute_success_simulates_and_marks_used_once() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
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
        assert!(
            result.receipt.signature_valid,
            "simulated receipt is signed"
        );

        let listed = list_capabilities(&env.conn).unwrap();
        assert!(
            listed[0].used_at.is_some(),
            "capability burned after execution"
        );
        assert!(listed[0].signature_valid, "signature survives execution");
        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts.len(), 1, "pending receipt upgraded in place");
    }

    #[test]
    fn execute_reuse_is_refused_without_connector() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
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
        let cap = propose_action(&env.conn, &env.keys, &json, None)
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
    fn execute_wrong_channel_is_refused_binding() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &env.keys,
            &cap.id,
            "other-connector",
            "acct-1",
            "production",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("capability bound to different channel")
        );
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_wrong_account_is_refused_binding() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &env.keys,
            &cap.id,
            "demo-connector",
            "acct-999",
            "production",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("capability bound to different channel")
        );
    }

    #[test]
    fn execute_wrong_environment_is_refused_binding() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        let result = execute_capability(
            &env.conn,
            &env.keys,
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
            Some("capability bound to different channel")
        );
    }

    #[test]
    fn execute_refuses_when_bound_channel_removed_from_registry() {
        let env = TestEnv::new();
        add_connector(&env.conn, "temp-connector", "acct-7", "sandbox").unwrap();
        let json = serde_json::to_string(&SoulAction {
            action_id: "act_3".to_string(),
            kind: "notes.create".to_string(),
            actor: "agent-1".to_string(),
            connector_id: "temp-connector".to_string(),
            account_id: "acct-7".to_string(),
            environment: "sandbox".to_string(),
            recipient: None,
            domain: None,
            amount: Some(10.0),
            currency: None,
            data_classes: vec![],
            reversible: true,
            confirmed_by_user: true,
            requested_scopes: vec!["notes:write".to_string()],
            payload_hash: String::new(),
        })
        .unwrap();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        remove_connector(&env.conn, "temp-connector", "acct-7", "sandbox").unwrap();
        let result = execute_capability(
            &env.conn,
            &env.keys,
            &cap.id,
            "temp-connector",
            "acct-7",
            "sandbox",
            &json,
        )
        .unwrap();
        assert!(!result.ok);
        assert_eq!(result.receipt.reason.as_deref(), Some("connector mismatch"));
        assert!(!result.receipt.connector_executed);
    }

    #[test]
    fn execute_expired_capability_is_refused() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose_action(&env.conn, &env.keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        set_expiry(&env, &cap.id, "2020-01-01T00:00:00+00:00");
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
        let cap = propose_action(&env.conn, &env.keys, &json, None)
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
        let proposal = propose_action(&env.conn, &env.keys, &denied, None).unwrap();
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

    #[test]
    fn property_propose_execute_roundtrip_replay_and_determinism() {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};
        let env = TestEnv::new();
        let mut rng = StdRng::seed_from_u64(0x51a0_1e13);
        let kinds = [
            "notes.create",
            "notes.read",
            "calendar.read",
            "slack.post",
            "email.send",
            "purchase.create",
        ];
        let mut nonces = std::collections::HashSet::new();
        let mut executed = 0usize;
        let mut denied = 0usize;

        for i in 0..60 {
            let action = SoulAction {
                action_id: format!("act_prop_{i}"),
                kind: kinds[rng.gen_range(0..kinds.len())].to_string(),
                actor: "agent-prop".to_string(),
                connector_id: "demo-connector".to_string(),
                account_id: "acct-1".to_string(),
                environment: "production".to_string(),
                recipient: None,
                domain: Some("shopping".to_string()),
                amount: Some(rng.gen_range(0.0..400.0)),
                currency: Some("USD".to_string()),
                data_classes: vec![],
                reversible: true,
                confirmed_by_user: true,
                requested_scopes: vec![],
                payload_hash: String::new(),
            };
            let json = serde_json::to_string(&action).unwrap();
            let proposal = propose_action(&env.conn, &env.keys, &json, Some(300)).unwrap();

            let Some(cap) = proposal.capability.as_ref() else {
                assert_eq!(proposal.decision.effect, Effect::Deny);
                denied += 1;
                continue;
            };
            assert!(nonces.insert(cap.nonce.clone()), "nonce must be unique");

            // Детерминизм хэша: тот же JSON → тот же payload_hash.
            let hash1 = payload_hash_of(&normalize_action(&json).unwrap());
            let hash2 = payload_hash_of(&normalize_action(&json).unwrap());
            assert_eq!(hash1, hash2);
            assert_eq!(cap.payload_hash, hash1);

            let result = execute_ok(&env, &cap.id, &json);
            assert!(
                result.ok,
                "allowed action must execute: {}",
                result.receipt.reason.clone().unwrap_or_default()
            );
            assert!(result.receipt.signature_valid, "receipt must be signed");
            assert!(result.receipt.connector_executed);
            executed += 1;

            // Повторное исполнение той же capability всегда отказывается.
            let again = execute_ok(&env, &cap.id, &json);
            assert!(!again.ok);
            assert_eq!(
                again.receipt.reason.as_deref(),
                Some("capability already used")
            );
        }

        assert!(
            executed >= 40,
            "most random allowed actions must execute: {executed}"
        );

        // Квитанции: по одной на propose (allow) + по одной refused-квитанции
        // на попытку повтора, + по одной на каждый deny-propose.
        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts.len(), executed * 2 + denied);
        for r in &receipts {
            assert!(r.signature_valid, "receipt must verify");
        }
    }

    // ---------- require_confirmation: поток подтверждения ----------

    #[test]
    fn held_capability_cannot_execute_before_confirmation() {
        let env = TestEnv::new();
        let json = env.purchase_600();
        let cap = propose(&env, &json).capability.unwrap();
        let result = execute_ok(&env, &cap.id, &json);
        assert!(!result.ok);
        assert_eq!(
            result.receipt.reason.as_deref(),
            Some("confirmation required")
        );
        assert!(!result.receipt.connector_executed);
        assert!(
            list_capabilities(&env.conn).unwrap()[0].used_at.is_none(),
            "refusal must not burn the capability"
        );
    }

    #[test]
    fn confirm_then_execute_held_capability_succeeds() {
        let env = TestEnv::new();
        let json = env.purchase_600();
        let cap = propose(&env, &json).capability.unwrap();

        let confirmed = confirm_capability(&env.conn, &env.keys, &cap.id).unwrap();
        assert!(confirmed.confirmed_by_user);
        assert!(confirmed.signature_valid);
        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts[0].status, GatewayStatus::Pending, "held → pending");
        assert!(receipts[0].signature_valid);

        let result = execute_ok(&env, &cap.id, &json);
        assert!(result.ok, "expected ok, got {:?}", result.receipt.reason);
        assert_eq!(result.receipt.status, GatewayStatus::Simulated);
        assert!(
            list_capabilities(&env.conn).unwrap()[0].used_at.is_some(),
            "capability burned after confirmed execution"
        );
    }

    #[test]
    fn confirm_allowed_capability_is_rejected() {
        let env = TestEnv::new();
        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        let err = confirm_capability(&env.conn, &env.keys, &cap.id).unwrap_err();
        assert!(
            err.contains("does not require confirmation"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn confirm_unknown_or_used_capability_is_rejected() {
        let env = TestEnv::new();
        let err = confirm_capability(&env.conn, &env.keys, "cap_nope").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");

        let json = env.allowed_action();
        let cap = propose(&env, &json).capability.unwrap();
        execute_ok(&env, &cap.id, &json);
        let err = confirm_capability(&env.conn, &env.keys, &cap.id).unwrap_err();
        assert!(err.contains("already used"), "unexpected error: {err}");
    }

    #[test]
    fn confirm_twice_is_rejected() {
        let env = TestEnv::new();
        let json = env.purchase_600();
        let cap = propose(&env, &json).capability.unwrap();
        confirm_capability(&env.conn, &env.keys, &cap.id).unwrap();
        let err = confirm_capability(&env.conn, &env.keys, &cap.id).unwrap_err();
        assert!(err.contains("already confirmed"), "unexpected error: {err}");
    }

    // ---------- redact: поддельный коннектор получает отредактированное действие ----------

    #[test]
    fn redacted_capability_executes_with_redacted_payload() {
        let env = TestEnv::new();
        create_policy(
            &env.conn,
            r#"{"id":"r_redact","priority":800,"when":{"eq":["action.kind","email.send"]},"effect":"redact","message":"m"}"#,
        )
        .unwrap();
        let json = env.allowed_action().replace("notes.create", "email.send");
        let cap = propose(&env, &json).capability.unwrap();

        let result = execute_ok(&env, &cap.id, &json);
        assert!(result.ok, "expected ok, got {:?}", result.receipt.reason);
        assert_eq!(result.receipt.status, GatewayStatus::Simulated);
        assert!(result.receipt.connector_executed);
        assert_eq!(result.receipt.decision_effect, Effect::Redact);
        let message = result.receipt.message.as_deref().unwrap();
        assert!(
            message.contains("redacted"),
            "unexpected message: {message}"
        );

        let stored: SoulAction = serde_json::from_str(
            &load_capability(&env.conn, &cap.id)
                .unwrap()
                .unwrap()
                .action_json,
        )
        .unwrap();
        let redacted = redact_variant(&stored);
        assert!(redacted.recipient.is_none() && redacted.amount.is_none());
        let expected_tx = fake_connector_execute(&redacted).transaction_id;
        assert!(
            message.contains(&expected_tx),
            "connector ran on the redacted variant"
        );
    }

    // ---------- Реестр каналов ----------

    #[test]
    fn registry_add_duplicate_and_remove() {
        let env = TestEnv::new();
        let added = add_connector(&env.conn, "pay-connector", "acct-9", "production").unwrap();
        assert_eq!(added.connector_id, "pay-connector");
        assert!(list_connectors(&env.conn)
            .unwrap()
            .iter()
            .any(|c| c == &added));

        add_connector(&env.conn, "pay-connector", "acct-9", "production").unwrap();
        let count: i64 = env
            .conn
            .query_row("SELECT COUNT(*) FROM gateway_connectors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5, "duplicate add is idempotent");

        let removed = remove_connector(&env.conn, "pay-connector", "acct-9", "production").unwrap();
        assert!(removed);
        assert!(!list_connectors(&env.conn)
            .unwrap()
            .iter()
            .any(|c| c.connector_id == "pay-connector"));
        let removed_again =
            remove_connector(&env.conn, "pay-connector", "acct-9", "production").unwrap();
        assert!(!removed_again, "nothing to remove");
    }

    #[test]
    fn registry_validates_and_limits() {
        let env = TestEnv::new();
        let err = add_connector(&env.conn, "  ", "acct-1", "production").unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected error: {err}");
        let err = add_connector(
            &env.conn,
            &"x".repeat(MAX_CHANNEL_FIELD_CHARS + 1),
            "acct-1",
            "production",
        )
        .unwrap_err();
        assert!(err.contains("exceeds"), "unexpected error: {err}");

        for i in 0..MAX_GATEWAY_CONNECTORS - 4 {
            add_connector(&env.conn, &format!("conn-{i}"), "acct-1", "production").unwrap();
        }
        let err = add_connector(&env.conn, "overflow", "acct-1", "production").unwrap_err();
        assert!(
            err.contains("Too many connector channels"),
            "unexpected error: {err}"
        );
    }

    // ---------- Хранилище ----------

    #[test]
    fn receipts_and_capabilities_are_listed_newest_first() {
        let env = TestEnv::new();
        propose_action(&env.conn, &env.keys, &env.allowed_action(), None).unwrap();
        let denied = env.allowed_action().replace("notes.create", "data.delete");
        propose_action(&env.conn, &env.keys, &denied, None).unwrap();

        let receipts = list_receipts(&env.conn).unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(
            receipts[0].created_at >= receipts[1].created_at,
            "newest first"
        );
        assert!(receipts.iter().any(|r| r.status == GatewayStatus::Denied));
        assert!(receipts.iter().any(|r| r.status == GatewayStatus::Pending));
        assert!(
            receipts.iter().all(|r| r.signature_valid),
            "all fresh receipts are signed"
        );
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
        let keys = crypto::ensure_device_keypair(&dir).unwrap();
        let json = allowed_action_json();
        let cap = propose_action(&conn, &keys, &json, None)
            .unwrap()
            .capability
            .unwrap();
        execute_capability(
            &conn,
            &keys,
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
