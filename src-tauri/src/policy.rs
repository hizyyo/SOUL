//! Детерминированный DSL политик (SESSION-11).
//!
//! Правила вида «сумма больше $500 требует подтверждения» решаются без
//! модели: безопасное типизированное подмножество условий (ULTRA_MVP §4.10,
//! мастер-план §12.5-12.8). Ядро — pure-функция `evaluate` над правилами и
//! типизированным `SoulAction`: никакого динамического eval, никаких сетевых
//! вызовов, никаких регулярок, никаких side effects во время оценки.
//!
//! Семантика решения: из всех включённых правил, чьи условия истинны,
//! побеждает правило с наибольшим приоритетом; при равенстве приоритетов —
//! с сильнейшим эффектом (решётка deny > require_confirmation > redact > allow);
//! при полном равенстве — стабильная детерминированная связка по id. Если ни
//! одно правило не сработало — `allow` (явный deny-rule для критичных действий
//! обязателен, см. «Проверка безопасности» SESSION-11).

use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db;

/// Максимальный размер сериализованного правила (символов).
pub const MAX_RULE_JSON_CHARS: usize = 4_096;
/// Верхняя граница приоритета правила.
pub const MAX_PRIORITY: i64 = 10_000;
/// Максимальное число правил в хранилище (защита списка).
pub const MAX_POLICY_RULES: usize = 500;
/// Максимальная длина сообщения правила.
pub const MAX_RULE_MESSAGE_CHARS: usize = 500;
/// Максимальная длина id правила.
pub const MAX_RULE_ID_CHARS: usize = 128;
/// Верхняя граница значения суммы, которую принимает DSL.
pub const MAX_AMOUNT: f64 = 1.0e12;

/// Эффект решения. P0-множество из ULTRA_MVP §4.10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Allow,
    Deny,
    RequireConfirmation,
    Redact,
}

impl Effect {
    /// Сила эффекта для решётки §12.3: deny > require_confirmation > redact > allow.
    pub fn rank(self) -> i32 {
        match self {
            Effect::Deny => 4,
            Effect::RequireConfirmation => 3,
            Effect::Redact => 2,
            Effect::Allow => 1,
        }
    }

    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Allow => "allow",
            Effect::Deny => "deny",
            Effect::RequireConfirmation => "require_confirmation",
            Effect::Redact => "redact",
        }
    }
}

/// Типизированное действие из §12.8 — вход для оценки политики.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoulAction {
    pub action_id: String,
    pub kind: String,
    pub actor: String,
    pub connector_id: String,
    pub account_id: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub recipient: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub data_classes: Vec<String>,
    #[serde(default)]
    pub reversible: bool,
    #[serde(default)]
    pub confirmed_by_user: bool,
    #[serde(default)]
    pub requested_scopes: Vec<String>,
    #[serde(default)]
    pub payload_hash: String,
}

/// Путь к полю действия. Нормализуется: `action.kind` ≡ `kind` (§12.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Kind,
    Amount,
    Currency,
    Domain,
    Recipient,
    Environment,
    Actor,
    ConnectorId,
    AccountId,
    Reversible,
    ConfirmedByUser,
    DataClasses,
}

const FIELDS: &[(&str, Field, FieldType)] = &[
    ("kind", Field::Kind, FieldType::Str),
    ("amount", Field::Amount, FieldType::Num),
    ("currency", Field::Currency, FieldType::Str),
    ("domain", Field::Domain, FieldType::Str),
    ("recipient", Field::Recipient, FieldType::Str),
    ("environment", Field::Environment, FieldType::Str),
    ("actor", Field::Actor, FieldType::Str),
    ("connector_id", Field::ConnectorId, FieldType::Str),
    ("account_id", Field::AccountId, FieldType::Str),
    ("reversible", Field::Reversible, FieldType::Bool),
    ("confirmed_by_user", Field::ConfirmedByUser, FieldType::Bool),
    ("data_classes", Field::DataClasses, FieldType::StrList),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Str,
    Num,
    Bool,
    StrList,
}

fn parse_field(path: &str) -> Option<(Field, FieldType)> {
    let normalized = path.strip_prefix("action.").unwrap_or(path);
    FIELDS
        .iter()
        .find(|(name, _, _)| *name == normalized)
        .map(|(_, field, field_type)| (*field, *field_type))
}

/// Типизированный литерал для сравнения с полем действия.
#[derive(Debug, Clone, PartialEq)]
enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
    StrList(Vec<String>),
}

fn literal_from_json(value: &Value) -> Option<Literal> {
    match value {
        Value::String(s) => Some(Literal::Str(s.clone())),
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()).map(Literal::Num),
        Value::Bool(b) => Some(Literal::Bool(*b)),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(s) => out.push(s.clone()),
                    _ => return None,
                }
            }
            Some(Literal::StrList(out))
        }
        _ => None,
    }
}

/// Атом условия: `{"eq": ["path", value]}` и аналоги.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Atom {
    Eq { eq: [Value; 2] },
    Neq { neq: [Value; 2] },
    In { r#in: [Value; 2] },
    Lt { lt: [Value; 2] },
    Lte { lte: [Value; 2] },
    Gt { gt: [Value; 2] },
    Gte { gte: [Value; 2] },
}

impl Atom {
    fn op_name(&self) -> &'static str {
        match self {
            Atom::Eq { .. } => "eq",
            Atom::Neq { .. } => "neq",
            Atom::In { .. } => "in",
            Atom::Lt { .. } => "lt",
            Atom::Lte { .. } => "lte",
            Atom::Gt { .. } => "gt",
            Atom::Gte { .. } => "gte",
        }
    }

    fn operands(&self) -> &[Value; 2] {
        match self {
            Atom::Eq { eq } => eq,
            Atom::Neq { neq } => neq,
            Atom::In { r#in } => r#in,
            Atom::Lt { lt } => lt,
            Atom::Lte { lte } => lte,
            Atom::Gt { gt } => gt,
            Atom::Gte { gte } => gte,
        }
    }

    /// Проверка литерала против поля; отдельно для `in` (список-семантика).
    fn validate(&self) -> Result<(Field, Literal), String> {
        let [path_v, lit_v] = self.operands();
        let path = path_v
            .as_str()
            .ok_or_else(|| format!("{}: path must be a string.", self.op_name()))?;
        let (field, field_type) =
            parse_field(path).ok_or_else(|| format!("unknown action field '{path}'."))?;
        let is_numeric_op = matches!(
            self,
            Atom::Lt { .. } | Atom::Lte { .. } | Atom::Gt { .. } | Atom::Gte { .. }
        );
        if is_numeric_op && field_type != FieldType::Num {
            return Err(format!(
                "{}: numeric comparison is only allowed on 'amount', got '{path}'.",
                self.op_name()
            ));
        }
        let is_in = matches!(self, Atom::In { .. });
        if is_in && field_type == FieldType::Bool {
            return Err(format!(
                "in: 'in' is not allowed on boolean field '{path}'."
            ));
        }
        let literal = literal_from_json(lit_v)
            .ok_or_else(|| format!("{}: unsupported literal for '{path}'.", self.op_name()))?;
        let type_ok = match (&literal, field_type) {
            (Literal::Str(_), FieldType::Str) => true,
            (Literal::Num(n), FieldType::Num) => n.abs() <= MAX_AMOUNT,
            (Literal::Bool(_), FieldType::Bool) => true,
            (Literal::StrList(items), FieldType::StrList) => !items.is_empty(),
            (Literal::StrList(items), FieldType::Str) if is_in => !items.is_empty(),
            _ => false,
        };
        if !type_ok {
            return Err(format!(
                "{}: literal type does not match field '{path}'.",
                self.op_name()
            ));
        }
        Ok((field, literal))
    }

    fn matches(&self, action: &SoulAction) -> bool {
        let Ok((field, literal)) = self.validate() else {
            return false;
        };
        let actual = field_value(action, field);
        match self {
            Atom::Eq { .. } => actual.as_ref() == Some(&literal),
            Atom::Neq { .. } => actual.as_ref() != Some(&literal),
            Atom::In { .. } => match (actual.as_ref(), &literal) {
                (Some(Literal::Str(value)), Literal::StrList(choices)) => {
                    choices.iter().any(|c| c == value)
                }
                (Some(Literal::StrList(actual_list)), Literal::StrList(choices)) => {
                    actual_list.iter().any(|item| choices.contains(item))
                }
                (Some(Literal::Num(n)), Literal::StrList(_)) => {
                    let _ = n;
                    false
                }
                _ => false,
            },
            Atom::Lt { .. } => num_compare(&actual, &literal, |a, b| a < b),
            Atom::Lte { .. } => num_compare(&actual, &literal, |a, b| a <= b),
            Atom::Gt { .. } => num_compare(&actual, &literal, |a, b| a > b),
            Atom::Gte { .. } => num_compare(&actual, &literal, |a, b| a >= b),
        }
    }
}

fn num_compare(actual: &Option<Literal>, literal: &Literal, cmp: fn(f64, f64) -> bool) -> bool {
    match (actual, literal) {
        (Some(Literal::Num(a)), Literal::Num(b)) => cmp(*a, *b),
        _ => false,
    }
}

fn field_value(action: &SoulAction, field: Field) -> Option<Literal> {
    match field {
        Field::Kind => Some(Literal::Str(action.kind.clone())),
        Field::Amount => action.amount.filter(|n| n.is_finite()).map(Literal::Num),
        Field::Currency => action.currency.clone().map(Literal::Str),
        Field::Domain => action.domain.clone().map(Literal::Str),
        Field::Recipient => action.recipient.clone().map(Literal::Str),
        Field::Environment => Some(Literal::Str(action.environment.clone())),
        Field::Actor => Some(Literal::Str(action.actor.clone())),
        Field::ConnectorId => Some(Literal::Str(action.connector_id.clone())),
        Field::AccountId => Some(Literal::Str(action.account_id.clone())),
        Field::Reversible => Some(Literal::Bool(action.reversible)),
        Field::ConfirmedByUser => Some(Literal::Bool(action.confirmed_by_user)),
        Field::DataClasses => Some(Literal::StrList(action.data_classes.clone())),
    }
}

/// Композиция условий: `all` / `any` / `not` и атомы.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
    Atom(Atom),
}

impl Condition {
    fn validate(&self) -> Result<(), String> {
        match self {
            Condition::All { all } => {
                if all.is_empty() {
                    return Err("'all' must contain at least one condition.".to_string());
                }
                for c in all {
                    c.validate()?;
                }
            }
            Condition::Any { any } => {
                if any.is_empty() {
                    return Err("'any' must contain at least one condition.".to_string());
                }
                for c in any {
                    c.validate()?;
                }
            }
            Condition::Not { not } => not.validate()?,
            Condition::Atom(atom) => {
                let _ = atom.validate()?;
            }
        }
        Ok(())
    }

    fn matches(&self, action: &SoulAction) -> bool {
        match self {
            Condition::All { all } => all.iter().all(|c| c.matches(action)),
            Condition::Any { any } => any.iter().any(|c| c.matches(action)),
            Condition::Not { not } => !not.matches(action),
            Condition::Atom(atom) => atom.matches(action),
        }
    }
}

/// Правило SoulRule (формат мастер-плана §12.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulRule {
    pub id: String,
    pub priority: i64,
    pub when: Condition,
    pub effect: Effect,
    #[serde(default)]
    pub message: String,
}

impl SoulRule {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Rule id must not be empty.".to_string());
        }
        if self.id.chars().count() > MAX_RULE_ID_CHARS {
            return Err(format!("Rule id exceeds {} characters.", MAX_RULE_ID_CHARS));
        }
        if !(0..=MAX_PRIORITY).contains(&self.priority) {
            return Err(format!(
                "Rule priority must be between 0 and {MAX_PRIORITY}."
            ));
        }
        if self.message.chars().count() > MAX_RULE_MESSAGE_CHARS {
            return Err(format!(
                "Rule message exceeds {} characters.",
                MAX_RULE_MESSAGE_CHARS
            ));
        }
        self.when.validate()?;
        Ok(())
    }
}

/// Результат оценки действия политиками.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    pub effect: Effect,
    pub rule_id: Option<String>,
    pub message: Option<String>,
}

/// Сериализуемая строка таблицы `policies`.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyRow {
    pub id: String,
    pub priority: i64,
    pub enabled: bool,
    pub rule_json: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn init_policies(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS policies (
            id TEXT PRIMARY KEY,
            priority INTEGER NOT NULL CHECK (priority >= 0 AND priority <= 10000),
            enabled INTEGER NOT NULL DEFAULT 1,
            rule_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS policy_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    seed_default_policies(conn)
}

/// Демонстрационные правила — один раз за жизнь хранилища (флаг в
/// `policy_meta`): удалённые пользователем правила не воскресают при рестарте.
/// Правила нужны UI-демо и Gateway-демо (SESSION-12).
fn seed_default_policies(conn: &Connection) -> SqlResult<()> {
    let seeded: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM policy_meta WHERE key = 'seeded')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v != 0)?;
    if seeded {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    let defaults: [(&str, &str, &str); 2] = [
        (
            "policy_high_value_confirmation",
            "900",
            r#"{
              "id": "policy_high_value_confirmation",
              "priority": 900,
              "when": {
                "all": [
                  { "eq": ["action.kind", "purchase.create"] },
                  { "gt": ["action.amount", 500] }
                ]
              },
              "effect": "require_confirmation",
              "message": "Purchases above $500 require confirmation."
            }"#,
        ),
        (
            "policy_destructive_denied",
            "1000",
            r#"{
              "id": "policy_destructive_denied",
              "priority": 1000,
              "when": {
                "any": [
                  { "eq": ["action.kind", "data.delete"] },
                  { "eq": ["action.kind", "account.delete"] }
                ]
              },
              "effect": "deny",
              "message": "Destructive deletes are denied."
            }"#,
        ),
    ];
    for (id, priority, rule_json) in defaults {
        conn.execute(
            "INSERT INTO policies (id, priority, enabled, rule_json, created_at, updated_at)
             VALUES (?1, ?2, 1, ?3, ?4, ?4)",
            params![id, priority, rule_json, now],
        )?;
    }
    conn.execute(
        "INSERT INTO policy_meta (key, value) VALUES ('seeded', ?1)",
        params![now],
    )?;
    Ok(())
}

/// Все правила, свежими по приоритету (desc), затем по id (стабильно).
pub fn list_policies(conn: &Connection) -> Result<Vec<PolicyRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, priority, enabled, rule_json, created_at, updated_at FROM policies ORDER BY priority DESC, id ASC")
        .map_err(|e| format!("policy list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PolicyRow {
                id: row.get(0)?,
                priority: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                rule_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("policy list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("policy row failed: {e}"))?);
    }
    Ok(out)
}

/// Создание правила из JSON (формат §12.5). Валидация до записи.
pub fn create_policy(conn: &Connection, rule_json: &str) -> Result<PolicyRow, String> {
    if rule_json.chars().count() > MAX_RULE_JSON_CHARS {
        return Err(format!("Rule exceeds {MAX_RULE_JSON_CHARS} characters."));
    }
    let rule: SoulRule = serde_json::from_str(rule_json)
        .map_err(|e| format!("Rule is not valid SoulRule JSON: {e}"))?;
    rule.validate()?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM policies", [], |r| r.get(0))
        .map_err(|e| format!("policy count failed: {e}"))?;
    if count >= MAX_POLICY_RULES as i64 {
        return Err(format!("Too many policies (limit {MAX_POLICY_RULES})."));
    }
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO policies (id, priority, enabled, rule_json, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?4, ?4)",
        params![&rule.id, rule.priority, rule_json, now],
    )
    .map_err(|e| format!("policy insert failed: {e}"))?;
    db::bump_policy_revision(conn).map_err(|e| format!("policy revision failed: {e}"))?;
    get_policy(conn, &rule.id).map_err(|e| format!("reload failed: {e}"))
}

fn get_policy(conn: &Connection, policy_id: &str) -> Result<PolicyRow, String> {
    conn.query_row(
        "SELECT id, priority, enabled, rule_json, created_at, updated_at
         FROM policies WHERE id = ?1",
        params![policy_id],
        |row| {
            Ok(PolicyRow {
                id: row.get(0)?,
                priority: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                rule_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )
    .map_err(|e| format!("policy lookup failed: {e}"))
}

/// Включение/выключение правила (без перезаписи JSON).
pub fn set_policy_enabled(
    conn: &Connection,
    policy_id: &str,
    enabled: bool,
) -> Result<PolicyRow, String> {
    let n = conn
        .execute(
            "UPDATE policies SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                if enabled { 1 } else { 0 },
                Utc::now().to_rfc3339(),
                policy_id
            ],
        )
        .map_err(|e| format!("policy update failed: {e}"))?;
    if n == 0 {
        return Err("Policy not found.".to_string());
    }
    db::bump_policy_revision(conn).map_err(|e| format!("policy revision failed: {e}"))?;
    get_policy(conn, policy_id)
}

pub fn delete_policy(conn: &Connection, policy_id: &str) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM policies WHERE id = ?1", params![policy_id])
        .map_err(|e| format!("policy delete failed: {e}"))?;
    if n == 0 {
        return Err("Policy not found.".to_string());
    }
    db::bump_policy_revision(conn).map_err(|e| format!("policy revision failed: {e}"))?;
    Ok(())
}

/// Оценка действия по включённым правилам. Чистая функция: никаких побочных
/// эффектов, детерминирована для одного набора правил.
pub fn evaluate(conn: &Connection, action: &SoulAction) -> Result<Decision, String> {
    let rows = list_policies(conn)?;
    let mut best: Option<(i64, Effect, String, String)> = None;
    for row in rows.iter().filter(|r| r.enabled) {
        let rule: SoulRule = match serde_json::from_str(&row.rule_json) {
            Ok(rule) => rule,
            Err(_) => continue, // битая строка не останавливает оценку остальных
        };
        if !rule.when.matches(action) {
            continue;
        }
        let better = match best {
            None => true,
            Some((best_priority, best_effect, _, _)) => {
                rule.priority > best_priority
                    || (rule.priority == best_priority && rule.effect.rank() > best_effect.rank())
            }
        };
        if better {
            best = Some((
                rule.priority,
                rule.effect,
                rule.id.clone(),
                rule.message.clone(),
            ));
        }
    }
    Ok(match best {
        Some((_, effect, rule_id, message)) => Decision {
            effect,
            rule_id: Some(rule_id.to_string()),
            message: Some(message.to_string()),
        },
        None => Decision {
            effect: Effect::Allow,
            rule_id: None,
            message: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use uuid::Uuid;

    struct TestEnv {
        dir: std::path::PathBuf,
        conn: Connection,
    }

    impl TestEnv {
        /// Чистый стол политик: дефолты засеяны, но удалены — тесты оценки
        /// не зависят от демо-правил. Флаг seed сохраняется.
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-policy-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            conn.execute("DELETE FROM policies", []).unwrap();
            TestEnv { dir, conn }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn parse_rule(json: &str) -> SoulRule {
        serde_json::from_str(json).unwrap()
    }

    fn mk_rule(id: &str, priority: i64, effect: Effect, when: &str) -> String {
        format!(
            r#"{{"id":"{id}","priority":{priority},"when":{when},"effect":"{}","message":"m"}}"#,
            effect.as_str()
        )
    }

    fn purchase(amount: Option<f64>) -> SoulAction {
        SoulAction {
            action_id: "act_1".to_string(),
            kind: "purchase.create".to_string(),
            actor: "agent-1".to_string(),
            connector_id: "conn_stripe".to_string(),
            account_id: "acct_1".to_string(),
            environment: "production".to_string(),
            recipient: None,
            domain: Some("shopping".to_string()),
            amount,
            currency: Some("USD".to_string()),
            data_classes: vec!["purchase_history".to_string()],
            reversible: false,
            confirmed_by_user: false,
            requested_scopes: vec!["purchase:write".to_string()],
            payload_hash: "h1".to_string(),
        }
    }

    // ---------- Property-тесты ----------

    #[test]
    fn property_evaluate_is_total_deterministic_and_never_panics() {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(0x5a01_13a5);
        let env = TestEnv::new();
        let conn = &env.conn;

        let kinds = [
            "notes.create",
            "purchase.create",
            "data.delete",
            "slack.post",
            "email.send",
            "account.delete",
            "calendar.read",
        ];
        let actors = ["agent-1", "agent-2", "user-1"];
        let connectors = ["conn_stripe", "conn_slack", "conn_ftp"];
        let accounts = ["acct_1", "acct_2", "acct_3"];
        let environments = ["production", "staging", "development"];
        let domains = [
            None,
            Some("shopping"),
            Some("finance"),
            Some("health"),
            Some("news"),
        ];
        let currencies = ["USD", "EUR", "RUB"];
        let effects = ["allow", "deny", "require_confirmation", "redact"];

        let atom_pool: Vec<&str> = vec![
            r#"{"eq":["action.kind","notes.create"]}"#,
            r#"{"neq":["action.kind","data.delete"]}"#,
            r#"{"in":["action.domain",["shopping","finance"]]}"#,
            r#"{"gt":["action.amount",500]}"#,
            r#"{"lte":["action.amount",1000]}"#,
            r#"{"eq":["action.connector_id","conn_stripe"]}"#,
            r#"{"eq":["action.reversible",true]}"#,
            r#"{"eq":["action.confirmed_by_user",false]}"#,
            r#"{"in":["action.environment",["production","staging"]]}"#,
            r#"{"in":["action.data_classes",["medical","financial"]]}"#,
        ];

        // Случайный, но валидный набор правил (детерминированный по сиду).
        let mut rule_count = 0usize;
        for _ in 0..24 {
            let effect = effects[rng.gen_range(0..effects.len())];
            let priority = rng.gen_range(0..=10_000);
            let when = rng.gen_range(0..4);
            let json = match when {
                0 => format!(
                    r#"{{"id":"prop_{rule_count}","priority":{priority},"when":{{"eq":["action.kind","{}"]}},"effect":"{effect}","message":"prop"}}"#,
                    kinds[rng.gen_range(0..kinds.len())]
                ),
                1 => format!(
                    r#"{{"id":"prop_{rule_count}","priority":{priority},"when":{{"any":[{}]}},"effect":"{effect}","message":"prop"}}"#,
                    atom_pool[rng.gen_range(0..atom_pool.len())]
                ),
                2 => format!(
                    r#"{{"id":"prop_{rule_count}","priority":{priority},"when":{{"not":{}}},"effect":"{effect}","message":"prop"}}"#,
                    atom_pool[rng.gen_range(0..atom_pool.len())]
                ),
                _ => format!(
                    r#"{{"id":"prop_{rule_count}","priority":{priority},"when":{{"all":[{}]}},"effect":"{effect}","message":"prop"}}"#,
                    atom_pool[rng.gen_range(0..atom_pool.len())]
                ),
            };
            if create_policy(conn, &json).is_ok() {
                rule_count += 1;
            }
        }
        assert!(rule_count >= 20, "seeded rules must be valid: {rule_count}");

        for i in 0..400 {
            let mut a = purchase(None);
            a.action_id = format!("act_prop_{i}");
            a.kind = kinds[rng.gen_range(0..kinds.len())].to_string();
            a.actor = actors[rng.gen_range(0..actors.len())].to_string();
            a.connector_id = connectors[rng.gen_range(0..connectors.len())].to_string();
            a.account_id = accounts[rng.gen_range(0..accounts.len())].to_string();
            a.environment = environments[rng.gen_range(0..environments.len())].to_string();
            a.domain = domains[rng.gen_range(0..domains.len())].map(|d| d.to_string());
            a.amount = Some(rng.gen_range(0.0..1e12));
            a.currency = Some(currencies[rng.gen_range(0..currencies.len())].to_string());
            a.recipient = if rng.gen_bool(0.3) {
                Some("external@corp.com".to_string())
            } else {
                None
            };
            a.data_classes = if rng.gen_bool(0.4) {
                vec!["medical".to_string()]
            } else {
                vec![]
            };

            let first = evaluate(conn, &a).unwrap();
            assert!(
                matches!(
                    first.effect,
                    Effect::Allow | Effect::Deny | Effect::RequireConfirmation | Effect::Redact
                ),
                "evaluation must always yield a defined effect"
            );
            let second = evaluate(conn, &a).unwrap();
            assert_eq!(first, second, "evaluate must be deterministic");
        }
    }

    // ---------- Валидация ----------

    #[test]
    fn validate_rejects_unknown_path() {
        let rule = parse_rule(&mk_rule(
            "r1",
            100,
            Effect::Allow,
            r#"{"eq":["action.hack","x"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(
            err.contains("unknown action field"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_rejects_unknown_operation() {
        let json =
            r#"{"id":"r1","priority":100,"when":{"like":["action.kind","x"]},"effect":"allow"}"#;
        assert!(serde_json::from_str::<SoulRule>(json).is_err());
    }

    #[test]
    fn validate_rejects_nonfinite_amount() {
        // 1e999 не представимо как f64 — парсинг JSON сам отвергает такое число.
        let json =
            r#"{"id":"r1","priority":100,"when":{"gt":["action.amount",1e999]},"effect":"allow"}"#;
        assert!(serde_json::from_str::<SoulRule>(json).is_err());
    }

    #[test]
    fn validate_rejects_wrong_literal_type() {
        let rule = parse_rule(&mk_rule(
            "r1",
            100,
            Effect::Allow,
            r#"{"eq":["action.amount","five"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(err.contains("does not match"), "unexpected error: {err}");

        let rule = parse_rule(&mk_rule(
            "r1",
            100,
            Effect::Allow,
            r#"{"eq":["action.reversible","yes"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(err.contains("does not match"), "unexpected error: {err}");
    }

    #[test]
    fn validate_rejects_empty_combinators() {
        let rule = parse_rule(&mk_rule("r1", 100, Effect::Allow, r#"{"all":[]}"#));
        let err = rule.validate().unwrap_err();
        assert!(err.contains("at least one"), "unexpected error: {err}");
    }

    #[test]
    fn validate_rejects_numeric_op_on_non_amount() {
        let rule = parse_rule(&mk_rule(
            "r1",
            100,
            Effect::Allow,
            r#"{"gt":["action.kind","a"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(
            err.contains("only allowed on 'amount'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_rejects_bad_priority_and_effect() {
        let rule = parse_rule(&mk_rule(
            "r1",
            -1,
            Effect::Allow,
            r#"{"eq":["action.kind","x"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(err.contains("priority"), "unexpected error: {err}");

        let rule = parse_rule(&mk_rule(
            "r1",
            20_000,
            Effect::Allow,
            r#"{"eq":["action.kind","x"]}"#,
        ));
        let err = rule.validate().unwrap_err();
        assert!(err.contains("priority"), "unexpected error: {err}");

        let json =
            r#"{"id":"r1","priority":100,"when":{"eq":["action.kind","x"]},"effect":"explode"}"#;
        assert!(serde_json::from_str::<SoulRule>(json).is_err());
    }

    #[test]
    fn validate_rejects_oversized_rule_json() {
        let big_message = "x".repeat(MAX_RULE_MESSAGE_CHARS + 1);
        let json = format!(
            r#"{{"id":"r1","priority":100,"when":{{"eq":["action.kind","x"]}},"effect":"allow","message":"{big_message}"}}"#
        );
        let rule = parse_rule(&json);
        let err = rule.validate().unwrap_err();
        assert!(err.contains("message exceeds"), "unexpected error: {err}");
    }

    // ---------- Семантика оценки ----------

    #[test]
    fn eval_gt_amount() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::RequireConfirmation,
                r#"{"gt":["action.amount",500]}"#,
            ),
        )
        .unwrap();

        let small = evaluate(conn, &purchase(Some(400.0))).unwrap();
        assert_eq!(small.effect, Effect::Allow);
        assert!(small.rule_id.is_none());

        let big = evaluate(conn, &purchase(Some(600.0))).unwrap();
        assert_eq!(big.effect, Effect::RequireConfirmation);
        assert_eq!(big.rule_id.as_deref(), Some("r1"));
    }

    #[test]
    #[ignore = "run through pnpm release:check; measures the production p95 budget"]
    fn release_policy_p95_is_under_5ms() {
        let env = TestEnv::new();
        create_policy(
            &env.conn,
            &mk_rule(
                "perf_rule",
                100,
                Effect::RequireConfirmation,
                r#"{"gt":["action.amount",500]}"#,
            ),
        )
        .unwrap();
        let action = purchase(Some(600.0));
        let _ = evaluate(&env.conn, &action); // warmup

        let mut samples: Vec<std::time::Duration> = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let decision = evaluate(&env.conn, &action).unwrap();
            assert_eq!(decision.effect, Effect::RequireConfirmation);
            samples.push(start.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
        eprintln!("release policy p95: {p95:?}");
        assert!(
            p95 < std::time::Duration::from_millis(5),
            "release policy p95 exceeded 5ms: {p95:?}"
        );
    }

    #[test]
    fn eval_kind_and_domain_match() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Deny,
                r#"{"all":[{"eq":["action.kind","email.send"]},{"eq":["action.domain","finance"]}]}"#,
            ),
        )
        .unwrap();

        let mut a = purchase(None);
        a.kind = "email.send".to_string();
        a.domain = Some("finance".to_string());
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Deny);

        a.domain = Some("sales".to_string());
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);
    }

    #[test]
    fn eval_recipient_and_sensitivity() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Redact,
                r#"{"all":[{"eq":["action.recipient","external@corp.com"]},{"in":["action.data_classes",["medical","financial"]]}]}"#,
            ),
        )
        .unwrap();

        let mut a = purchase(None);
        a.recipient = Some("external@corp.com".to_string());
        a.data_classes = vec!["notes".to_string(), "medical".to_string()];
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Redact);

        a.data_classes = vec!["notes".to_string()];
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);

        a.recipient = None;
        a.data_classes = vec!["medical".to_string()];
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);
    }

    #[test]
    fn eval_reversible_and_confirmed() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Deny,
                r#"{"all":[{"eq":["action.reversible",false]},{"eq":["action.confirmed_by_user",false]}]}"#,
            ),
        )
        .unwrap();

        let mut a = purchase(None);
        a.reversible = false;
        a.confirmed_by_user = false;
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Deny);

        a.confirmed_by_user = true;
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);
    }

    #[test]
    fn eval_environment_in() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Deny,
                r#"{"in":["action.environment",["production","staging"]]}"#,
            ),
        )
        .unwrap();

        let mut a = purchase(None);
        a.environment = "production".to_string();
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Deny);
        a.environment = "development".to_string();
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);
    }

    #[test]
    fn eval_all_any_not_nesting() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Deny,
                r#"{"all":[{"eq":["action.kind","transfer.create"]},{"not":{"any":[{"eq":["action.currency","USD"]},{"eq":["action.amount",10.0]}]}}]}"#,
            ),
        )
        .unwrap();

        let mut a = purchase(Some(50.0));
        a.kind = "transfer.create".to_string();
        a.currency = Some("EUR".to_string());
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Deny);

        a.currency = Some("USD".to_string());
        assert_eq!(evaluate(conn, &a).unwrap().effect, Effect::Allow);
    }

    #[test]
    fn eval_priority_ordering() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "low",
                100,
                Effect::Allow,
                r#"{"eq":["action.kind","purchase.create"]}"#,
            ),
        )
        .unwrap();
        create_policy(
            conn,
            &mk_rule(
                "high",
                900,
                Effect::RequireConfirmation,
                r#"{"gt":["action.amount",400]}"#,
            ),
        )
        .unwrap();

        let decision = evaluate(conn, &purchase(Some(600.0))).unwrap();
        assert_eq!(decision.rule_id.as_deref(), Some("high"));
        assert_eq!(decision.effect, Effect::RequireConfirmation);
    }

    #[test]
    fn eval_deny_beats_allow_on_priority_tie() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "allow_all",
                500,
                Effect::Allow,
                r#"{"eq":["action.kind","purchase.create"]}"#,
            ),
        )
        .unwrap();
        create_policy(
            conn,
            &mk_rule(
                "deny_big",
                500,
                Effect::Deny,
                r#"{"gt":["action.amount",100]}"#,
            ),
        )
        .unwrap();

        let decision = evaluate(conn, &purchase(Some(200.0))).unwrap();
        assert_eq!(decision.effect, Effect::Deny);
        assert_eq!(decision.rule_id.as_deref(), Some("deny_big"));
    }

    #[test]
    fn eval_no_match_is_allow() {
        let env = TestEnv::new();
        let conn = &env.conn;
        let decision = evaluate(conn, &purchase(Some(100.0))).unwrap();
        assert_eq!(decision.effect, Effect::Allow);
        assert!(decision.rule_id.is_none());
        assert!(decision.message.is_none());
    }

    #[test]
    fn eval_disabled_rules_skipped() {
        let env = TestEnv::new();
        let conn = &env.conn;
        let created = create_policy(
            conn,
            &mk_rule(
                "r1",
                100,
                Effect::Deny,
                r#"{"eq":["action.kind","purchase.create"]}"#,
            ),
        )
        .unwrap();
        assert_eq!(
            evaluate(conn, &purchase(Some(1.0))).unwrap().effect,
            Effect::Deny
        );

        set_policy_enabled(conn, &created.id, false).unwrap();
        assert_eq!(
            evaluate(conn, &purchase(Some(1.0))).unwrap().effect,
            Effect::Allow
        );
    }

    #[test]
    fn eval_broken_rule_does_not_stop_evaluation() {
        let env = TestEnv::new();
        let conn = &env.conn;
        create_policy(
            conn,
            &mk_rule(
                "ok",
                100,
                Effect::Deny,
                r#"{"eq":["action.kind","purchase.create"]}"#,
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO policies (id, priority, enabled, rule_json, created_at, updated_at)
             VALUES ('broken', 999, 1, 'not json at all', 't', 't')",
            [],
        )
        .unwrap();

        let decision = evaluate(conn, &purchase(Some(1.0))).unwrap();
        assert_eq!(decision.effect, Effect::Deny);
        assert_eq!(decision.rule_id.as_deref(), Some("ok"));
    }

    // ---------- Хранилище ----------

    #[test]
    fn db_crud_and_seed_once() {
        let env = TestEnv::new();
        let conn = &env.conn;
        assert!(
            list_policies(conn).unwrap().is_empty(),
            "TestEnv clears seeds"
        );

        create_policy(
            conn,
            &mk_rule(
                "custom",
                50,
                Effect::Redact,
                r#"{"eq":["action.kind","x"]}"#,
            ),
        )
        .unwrap();
        assert_eq!(list_policies(conn).unwrap().len(), 1);

        let created = create_policy(
            conn,
            &mk_rule("dup", 50, Effect::Redact, r#"{"eq":["action.kind","x"]}"#),
        )
        .unwrap();
        let err = create_policy(conn, &created.rule_json).unwrap_err();
        assert!(
            err.contains("insert failed") || err.contains("UNIQUE"),
            "unexpected error: {err}"
        );

        delete_policy(conn, "custom").unwrap();
        assert_eq!(list_policies(conn).unwrap().len(), 1, "only 'dup' remains");

        let err = delete_policy(conn, "custom").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");

        // Свежее хранилище: ровно 2 дефолта, повторный init ничего не добавляет.
        let dir = std::env::temp_dir().join(format!("soul-policy-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = init_db(&dir).unwrap();
        let seeded = list_policies(&conn).unwrap();
        assert_eq!(seeded.len(), 2, "defaults seeded on first init");
        assert!(
            seeded[0].priority >= seeded[1].priority,
            "ordered by priority desc"
        );
        assert_eq!(
            list_policies(&conn).unwrap().len(),
            2,
            "seeding is idempotent"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_rejects_unknown_field_and_bad_json() {
        let env = TestEnv::new();
        let conn = &env.conn;
        let err = create_policy(
            conn,
            &mk_rule("r1", 100, Effect::Allow, r#"{"eq":["action.nope","x"]}"#),
        )
        .unwrap_err();
        assert!(
            err.contains("unknown action field"),
            "unexpected error: {err}"
        );

        let err = create_policy(conn, "[]").unwrap_err();
        assert!(
            err.contains("not valid SoulRule JSON"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wipe_all_clears_policies_and_seed_flag() {
        let dir = std::env::temp_dir().join(format!("soul-policy-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = init_db(&dir).unwrap();
        assert_eq!(list_policies(&conn).unwrap().len(), 2, "defaults seeded");

        crate::db::wipe_all(&conn).unwrap();
        assert!(list_policies(&conn).unwrap().is_empty());

        // После полной очистки флаг seed тоже сброшен — следующий init засеет заново.
        drop(conn);
        let conn = init_db(&dir).unwrap();
        assert_eq!(list_policies(&conn).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
