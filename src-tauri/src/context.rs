//! Детерминированный компилятор контекста (Rust-порт SESSION-07 TS-модуля).
//!
//! Чистые функции: без сети, без модели, без часов и случайности — одинаковое
//! состояние SOUL + одинаковый запрос всегда дают одинаковый пак. Это
//! единственный источник истины для MCP-сервера (`soul.get_context`); TS-копия
//! используется только в UI и держится синхронной (golden-тесты по обеим
//! сторонам). Семантика и сериализация совпадают с `src/data/context.ts`.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::db;

pub const CONTEXT_POLICY_VERSION: &str = "soul-context-policy/1";
pub const CONTEXT_STANDARD_TOKENS: u64 = 900;
pub const CONTEXT_HARD_MAX_TOKENS: u64 = 3000;

/// Оценочная стоимость входа в модель (USD за 1 000 входных токенов).
/// Нейтральный консервативный профиль: приложение не вызывает модель само
/// (компилятор детерминированный), эта цена оценивает стоимость контекста,
/// который SOUL добавляет во внешний чат-модели. Настраивается константой;
/// значение — оценка, не фактура.
pub const COST_USD_PER_1K_INPUT_TOKENS: f64 = 0.005;

/// Детерминированная оценка стоимости входных токенов контекста в USD.
pub fn cost_estimate_usd(input_tokens: u64) -> f64 {
    (input_tokens as f64 / 1000.0) * COST_USD_PER_1K_INPUT_TOKENS
}

/// Пределы запроса к контексту на границе локального IPC (MCP/bridge):
/// защита от oversized-строк и массивов фильтров. Значения зеркалят лимит
/// bridge-задачи; запросы сверх них отклоняются ещё до компиляции.
pub const QUERY_MAX_TEXT_CHARS: usize = 8_000;
pub const QUERY_MAX_FILTER_ENTRIES: usize = 64;
pub const QUERY_MAX_FILTER_TOTAL: usize = 200;
pub const QUERY_MAX_ENTRY_CHARS: usize = 256;
pub const QUERY_MAX_TIMESTAMP_CHARS: usize = 64;

/// Валидация запроса на границе IPC: детерминированная, без побочных
/// эффектов. Точный список ошибок, а не молчаливое усечение, — клиент
/// получает понятный ответ.
pub fn validate_query(query: &ContextQuery) -> Result<(), String> {
    if query.text.chars().count() > QUERY_MAX_TEXT_CHARS {
        return Err(format!(
            "Query text is too long (limit {QUERY_MAX_TEXT_CHARS} characters)."
        ));
    }
    let mut total = 0usize;
    let dimensions = [
        ("domains", &query.domains),
        ("projects", &query.projects),
        ("people", &query.people),
        ("channels", &query.channels),
        ("sensitivity", &query.sensitivity),
        ("statuses", &query.statuses),
    ];
    for (name, items) in dimensions {
        if items.len() > QUERY_MAX_FILTER_ENTRIES {
            return Err(format!(
                "Query {name} filter is too large (limit {QUERY_MAX_FILTER_ENTRIES} entries)."
            ));
        }
        total += items.len();
        for entry in items {
            if entry.chars().count() > QUERY_MAX_ENTRY_CHARS {
                return Err(format!("Query {name} entry is too long."));
            }
        }
    }
    if total > QUERY_MAX_FILTER_TOTAL {
        return Err(format!(
            "Query filters exceed {QUERY_MAX_FILTER_TOTAL} total entries."
        ));
    }
    for (name, value) in [("since", &query.since), ("until", &query.until)] {
        if let Some(v) = value {
            if v.chars().count() > QUERY_MAX_TIMESTAMP_CHARS {
                return Err(format!("Query {name} is too long."));
            }
        }
    }
    Ok(())
}

const DEFAULT_ALLOWED_STATUSES: [&str; 1] = ["active"];
const DEFAULT_ALLOWED_SENSITIVITY: [&str; 4] = ["public", "internal", "private", "sensitive"];
const ALL_SENSITIVITY: [&str; 5] = ["public", "internal", "private", "sensitive", "restricted"];

/// Приоритет типов: границы всегда выше предпочтений и фактов.
fn priority_tier(entity_type: &str) -> u64 {
    match entity_type {
        "boundary" => 4,
        "decision" => 3,
        "goal" => 2,
        "preference" => 1,
        "fact" => 1,
        _ => 0,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextEntity {
    pub id: String,
    pub soul_id: String,
    pub entity_type: String,
    pub status: String,
    pub data: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextQuery {
    pub text: String,
    /// Разрешённые области; пустой массив = без ограничения по этому измерению.
    pub domains: Vec<String>,
    pub projects: Vec<String>,
    pub people: Vec<String>,
    pub channels: Vec<String>,
    /// Разрешённые уровни чувствительности; пустой массив = все, кроме restricted.
    pub sensitivity: Vec<String>,
    /// Разрешённые статусы; пустой массив = только active.
    pub statuses: Vec<String>,
    /// ISO-строки; сущности вне окна [since, until] исключаются.
    pub since: Option<String>,
    pub until: Option<String>,
    /// Бюджет токенов: от 1 до 3000, по умолчанию 900.
    pub max_tokens: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextItem {
    pub id: String,
    pub entity_type: String,
    pub status: String,
    pub claim: String,
    pub evidence: String,
    pub sensitivity: String,
    pub domains: Vec<String>,
    pub relevance: i64,
    pub priority: u64,
    pub confidence: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextConflict {
    pub a: String,
    pub b: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPack {
    pub items: Vec<ContextItem>,
    pub conflicts: Vec<ContextConflict>,
    /// ID сущностей, удалённых как замещённые дубликаты (нет в паке).
    pub superseded_ids: Vec<String>,
    pub policy_version: String,
    /// Детерминированная версия состояния: хэш включённых сущностей.
    pub state_version: String,
    pub max_tokens: u64,
    pub token_estimate: u64,
    pub serialized: String,
}

fn is_cjk(code: u32) -> bool {
    (0x4e00..=0x9fff).contains(&code) // CJK Unified
        || (0x3400..=0x4dbf).contains(&code) // CJK Ext A
        || (0x20000..=0x2fa1f).contains(&code) // CJK Ext B+
        || (0x3040..=0x30ff).contains(&code) // Hiragana + Katakana
        || (0xac00..=0xd7af).contains(&code) // Hangul
        || (0xff00..=0xffef).contains(&code) // Fullwidth
        || (0x3000..=0x303f).contains(&code) // CJK punctuation
}

/// Консервативная детерминированная оценка токенов без модели.
/// CJK-символ ~1 токен, остальные символы ~1/3 токена. Сложение выполняется
/// по одному символу (как в TS), чтобы float-погрешности совпадали.
pub fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    let mut units = 0.0f64;
    for ch in text.chars() {
        units += if is_cjk(ch as u32) { 1.0 } else { 1.0 / 3.0 };
    }
    units.ceil() as u64
}

/// Точный целочисленный эквивалент оценки токенов (без float): счётчик CJK
/// и остальных символов. `tokens()` даёт ровно тот же результат, что
/// `estimate_tokens` для любой строки, но позволяет инкрементально считать
/// оценку по частям (SESSION-14: упаковка O(n) вместо O(n²)).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Units {
    cjk: u64,
    latin: u64,
}

impl Units {
    fn of(text: &str) -> Units {
        let mut units = Units::default();
        for ch in text.chars() {
            units.add_char(ch);
        }
        units
    }

    fn add_char(&mut self, ch: char) {
        if is_cjk(ch as u32) {
            self.cjk += 1;
        } else {
            self.latin += 1;
        }
    }

    fn add_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.add_char(ch);
        }
    }

    fn tokens(self) -> u64 {
        self.cjk + self.latin.div_ceil(3)
    }
}

/// Единица измерения одного символа-разделителя (не CJK) между частями.
const SEP_UNITS: Units = Units { cjk: 0, latin: 1 };

/// 32-битный FNV-1a поверх UTF-16 code units (совпадает с TS charCodeAt).
/// Детерминированный, без потери точности (wrapping_mul = imul).
pub fn hash_string(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for unit in text.encode_utf16() {
        hash ^= unit as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// Число с разделителями тысяч как `toLocaleString('en-US')`.
pub fn format_tokens(value: u64) -> String {
    let s = value.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.char_indices() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn parse_data(data: &str) -> serde_json::Value {
    serde_json::from_str(data).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
fn obj_of(data: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    parse_data(data).as_object().cloned()
}

#[cfg(test)]
fn str_of(data: &str, key: &str) -> Option<String> {
    obj_of(data)
        .and_then(|o| o.get(key).cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn domain_for_question(question_id: &str) -> &'static str {
    if question_id.starts_with("goal_") {
        "goals"
    } else if question_id.starts_with("bound_") {
        "boundaries"
    } else if question_id.starts_with("dec_") {
        "decisions"
    } else if question_id.starts_with("write_") {
        "writing"
    } else if question_id.starts_with("text_") {
        "personal"
    } else {
        "preferences"
    }
}

/// Разобранная сущность (SESSION-14): один JSON-парсинг на сущность вместо
/// десятка повторных в фильтрующем цикле. Поля извлекаются один раз и
/// переиспользуются и в фильтрах, и при сборке пакета.
struct ParsedEntity {
    id: String,
    entity_type: String,
    status: String,
    created_at: String,
    updated_at: String,
    claim: String,
    evidence: String,
    sensitivity: String,
    domains: Vec<String>,
    projects: Vec<String>,
    people: Vec<String>,
    channels: Vec<String>,
    confidence: f64,
    question_id: Option<String>,
    value_key: String,
}

impl ParsedEntity {
    fn from(entity: &ContextEntity) -> ParsedEntity {
        let data = parse_data(&entity.data);
        let obj = data.as_object();
        let get_str = |key: &str| -> String {
            obj.and_then(|o| o.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        let scope_list = |dim: &str| -> Vec<String> {
            obj.and_then(|o| o.get("scope"))
                .and_then(|s| s.as_object())
                .and_then(|o| o.get(dim))
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };
        let sensitivity = get_str("sensitivity");
        let sensitivity = if ALL_SENSITIVITY.contains(&sensitivity.as_str()) {
            sensitivity
        } else {
            "internal".to_string()
        };
        let domains_raw = scope_list("domains");
        let raw_question_id = get_str("questionId");
        let question_id = (!raw_question_id.is_empty()).then_some(raw_question_id);
        let domains = if !domains_raw.is_empty() {
            domains_raw
        } else if let Some(question_id) = question_id.as_deref() {
            vec![domain_for_question(question_id).to_string()]
        } else {
            Vec::new()
        };
        let confidence = obj
            .and_then(|o| o.get("confidence"))
            .and_then(|v| v.as_f64())
            .map(|c| {
                if c.is_finite() {
                    c.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let value_key = obj
            .and_then(|o| o.get("value"))
            .map(serde_json::to_string)
            .transpose()
            .unwrap_or_default()
            .unwrap_or_else(|| "null".to_string());
        ParsedEntity {
            id: entity.id.clone(),
            entity_type: entity.entity_type.clone(),
            status: entity.status.clone(),
            created_at: entity.created_at.clone(),
            updated_at: entity.updated_at.clone(),
            claim: get_str("claim"),
            evidence: get_str("evidence"),
            sensitivity,
            domains,
            projects: scope_list("projects"),
            people: scope_list("people"),
            channels: scope_list("channels"),
            confidence,
            question_id,
            value_key,
        }
    }
}

/// Токенизация для релевантности: строчные unicode-слова.
fn tokenize(text: &str) -> Vec<String> {
    let lower: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Релевантность сущности запросу: термин в claim ×2, в evidence ×1.
/// Пустой запрос — релевантность 0 для всех (пак собирается без текста).
/// Термы запроса дедуплицируются (как Set в TS): повтор слова в запросе не
/// удваивает счёт — иначе счёт и порядок отличались бы от TS-копии.
/// Используется тестами parity с TS-копией; боевой путь — `relevance_of_terms`.
#[cfg(test)]
pub fn relevance_of(entity: &ContextEntity, query_text: &str) -> i64 {
    if query_text.trim().is_empty() {
        return 0;
    }
    let mut terms = tokenize(query_text);
    terms.sort();
    terms.dedup();
    if terms.is_empty() {
        return 0;
    }
    let claim_text = str_of(&entity.data, "claim").unwrap_or_else(|| entity.data.clone());
    let evidence_text = str_of(&entity.data, "evidence").unwrap_or_default();
    let claim = tokenize(&claim_text);
    let evidence = tokenize(&evidence_text);
    let mut score = 0;
    for term in terms {
        if claim.iter().any(|t| t == &term) {
            score += 2;
        }
        if evidence.iter().any(|t| t == &term) {
            score += 1;
        }
    }
    score
}

/// Дедуплицированные отсортированные термы запроса (кэш на один компиляции):
/// токенизация запроса выполняется один раз, а не на каждую сущность.
fn query_terms(query_text: &str) -> Vec<String> {
    let mut terms = tokenize(query_text);
    terms.sort();
    terms.dedup();
    terms
}

/// Релевантность по заранее подготовленным термам запроса (SESSION-14):
/// семантика та же, что у `relevance_of`, но без повторной токенизации
/// запроса на каждую сущность и без повторного парсинга данных.
fn relevance_of_terms(parsed: &ParsedEntity, terms: &[String]) -> i64 {
    if terms.is_empty() {
        return 0;
    }
    // Не строим Vec<String> для claim и evidence каждой сущности. Для 10k
    // записей это были сотни тысяч коротких аллокаций; проход по lower-case
    // строке сохраняет слово-в-слово семантику tokenize/any выше.
    fn matching_terms(text: &str, terms: &[String]) -> usize {
        let lower: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
        let mut matched = vec![false; terms.len()];
        for token in lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|token| !token.is_empty())
        {
            for (index, term) in terms.iter().enumerate() {
                if !matched[index] && token == term {
                    matched[index] = true;
                }
            }
        }
        matched.into_iter().filter(|found| *found).count()
    }

    (matching_terms(&parsed.claim, terms) * 2 + matching_terms(&parsed.evidence, terms)) as i64
}

/// Ограничение по одному измерению области: пусто = без ограничения.
fn matches_scope_dimension(entity_values: &[String], allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    allowed.iter().any(|a| entity_values.contains(a))
}

/// Проверка вхождения сущности в окно времени (RFC3339/ISO).
/// Непарсибельные даты ведут себя как всегда-видимые (как Date.parse в TS).
fn in_time_window(created_at: &str, since: &Option<String>, until: &Option<String>) -> bool {
    if since.is_none() && until.is_none() {
        return true;
    }
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(created_at) else {
        return true;
    };
    if let Some(s) = since {
        if let Ok(sd) = chrono::DateTime::parse_from_rfc3339(s) {
            if ts < sd {
                return false;
            }
        }
    }
    if let Some(u) = until {
        if let Ok(ud) = chrono::DateTime::parse_from_rfc3339(u) {
            if ts > ud {
                return false;
            }
        }
    }
    true
}

struct DedupResult<'a> {
    kept: Vec<&'a ParsedEntity>,
    superseded_ids: Vec<String>,
}

/// Удаление замещённых дубликатов: из сущностей с одинаковым questionId
/// (перекомпиляция калибровки) остаётся только самая свежая по updated_at.
/// Сущности без questionId (legacy) не трогаются. Старые ответы помечаются
/// в superseded_ids — они не попадают в пак, но видны в отчёте.
fn dedupe_superseded(entities: &[ParsedEntity]) -> DedupResult<'_> {
    let mut by_question: std::collections::HashMap<&str, Vec<&ParsedEntity>> =
        std::collections::HashMap::new();
    let mut standalone: Vec<&ParsedEntity> = Vec::new();
    for entity in entities {
        match entity.question_id.as_deref() {
            None => standalone.push(entity),
            Some(q) => {
                by_question.entry(q).or_default().push(entity);
            }
        }
    }
    let mut kept = standalone;
    let mut superseded_ids: Vec<String> = Vec::new();
    for (_, mut group) in by_question {
        group.sort_by(|a, b| {
            let recency = b.updated_at.cmp(&a.updated_at);
            if recency != std::cmp::Ordering::Equal {
                return recency;
            }
            a.id.cmp(&b.id)
        });
        let newest = group[0];
        kept.push(newest);
        for old in group.iter().skip(1) {
            superseded_ids.push(old.id.clone());
        }
    }
    DedupResult {
        kept,
        superseded_ids,
    }
}

/// Явные конфликты: разные ответы на один калибровочный вопрос. Считается по
/// ВСЕМ входным сущностям (до дедупликации), поэтому повторный ответ с другим
/// значением даёт пару (старое значение → новое). Пары детерминированы
/// (сортировка по id), каждая сущность участвует максимум в одной паре.
/// Ключ значения — каноническая форма serde_json (отсортированные ключи
/// объектов), что семантически эквивалентно JSON.stringify в TS.
fn detect_conflicts(entities: &[ParsedEntity]) -> Vec<ContextConflict> {
    let mut by_question: std::collections::HashMap<&str, Vec<&ParsedEntity>> =
        std::collections::HashMap::new();
    for entity in entities {
        let Some(q) = entity.question_id.as_deref() else {
            continue;
        };
        by_question.entry(q).or_default().push(entity);
    }
    let mut conflicts: Vec<ContextConflict> = Vec::new();
    for (question_id, group) in by_question {
        let mut by_value: std::collections::HashMap<&str, Vec<&ParsedEntity>> =
            std::collections::HashMap::new();
        for entity in &group {
            by_value
                .entry(entity.value_key.as_str())
                .or_default()
                .push(*entity);
        }
        if by_value.len() < 2 {
            continue;
        }
        let mut representatives: Vec<&ParsedEntity> = Vec::new();
        for (_, group) in by_value {
            representatives.push(group[0]);
        }
        representatives.sort_by(|a, b| a.id.cmp(&b.id));
        let anchor = &representatives[0];
        for other in representatives.iter().skip(1) {
            conflicts.push(ContextConflict {
                a: anchor.id.clone(),
                b: other.id.clone(),
                reason: format!("Same calibration question ({question_id}) with different answers"),
            });
        }
    }
    conflicts
}

fn to_context_item(parsed: &ParsedEntity, relevance: i64) -> ContextItem {
    ContextItem {
        id: parsed.id.clone(),
        entity_type: parsed.entity_type.clone(),
        status: parsed.status.clone(),
        claim: parsed.claim.clone(),
        evidence: parsed.evidence.clone(),
        sensitivity: parsed.sensitivity.clone(),
        domains: parsed.domains.clone(),
        relevance,
        priority: priority_tier(&parsed.entity_type),
        confidence: parsed.confidence,
        updated_at: parsed.updated_at.clone(),
    }
}

/// Полный детерминированный порядок: приоритет → релевантность → уверенность → свежесть → id.
fn compare_items(a: &ContextItem, b: &ContextItem) -> std::cmp::Ordering {
    b.priority
        .cmp(&a.priority)
        .then_with(|| b.relevance.cmp(&a.relevance))
        .then_with(|| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| b.updated_at.cmp(&a.updated_at))
        .then_with(|| a.id.cmp(&b.id))
}

fn serialize_pack_body_parts(
    body: &str,
    conflicts: &[ContextConflict],
    superseded_ids: &[String],
    state_version: &str,
    item_count: usize,
    max_tokens: u64,
) -> String {
    let mut parts = vec![
        "SOUL CONTEXT".to_string(),
        format!("policy: {CONTEXT_POLICY_VERSION}"),
        format!("state: {state_version}"),
        format!("tokens: X of {}", format_tokens(max_tokens)),
        format!("entities: {item_count}"),
        body.to_string(),
    ];
    if !conflicts.is_empty() {
        parts.push("CONFLICTS:".to_string());
        for c in conflicts {
            parts.push(format!("- {} vs {}: {}", c.a, c.b, c.reason));
        }
    }
    if !superseded_ids.is_empty() {
        parts.push(format!("SUPERSEDED: {}", superseded_ids.join(", ")));
    }
    parts.join("\n")
}

/// Главная детерминированная функция: компилирует минимальный разрешённый
/// контекст задачи. Текстовый запрос отсекает нерелевантные сущности полностью
/// (relevance == 0 не попадает в пак); бюджет никогда не превышается.
pub fn compile_context(entities: &[ContextEntity], query: &ContextQuery) -> ContextPack {
    let raw = query
        .max_tokens
        .unwrap_or(CONTEXT_STANDARD_TOKENS as f64)
        .floor();
    let max_tokens = if raw.is_finite() {
        (raw.max(1.0).min(CONTEXT_HARD_MAX_TOKENS as f64)) as u64
    } else {
        CONTEXT_STANDARD_TOKENS
    };
    let allowed_sensitivity: Vec<String> = if query.sensitivity.is_empty() {
        DEFAULT_ALLOWED_SENSITIVITY
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        query.sensitivity.clone()
    };
    let allowed_statuses: Vec<String> = if query.statuses.is_empty() {
        DEFAULT_ALLOWED_STATUSES
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        query.statuses.clone()
    };
    let query_text = query.text.trim().to_string();
    // Термы запроса токенизируются один раз на всю компиляцию (SESSION-14).
    let terms = if query_text.is_empty() {
        Vec::new()
    } else {
        query_terms(&query_text)
    };

    // Конфликты считаются по сырым данным: повторные ответы на один вопрос с
    // другим значением дают пару (старое значение → новое) ещё до дедупликации.
    // Парсим данные ровно один раз; дедупликация, конфликты и фильтры дальше
    // используют эту подготовленную структуру, а не повторно deserializing JSON.
    let parsed_entities: Vec<ParsedEntity> = entities.iter().map(ParsedEntity::from).collect();
    let mut conflicts = detect_conflicts(&parsed_entities);
    conflicts.sort_by(|x, y| format!("{}|{}", x.a, x.b).cmp(&format!("{}|{}", y.a, y.b)));

    let dedup = dedupe_superseded(&parsed_entities);
    let mut superseded_ids = dedup.superseded_ids;
    superseded_ids.sort();

    let mut items: Vec<ContextItem> = Vec::new();
    for parsed in dedup.kept {
        if !allowed_statuses.contains(&parsed.status) {
            continue;
        }
        if !allowed_sensitivity.contains(&parsed.sensitivity) {
            continue;
        }
        if !matches_scope_dimension(&parsed.projects, &query.projects) {
            continue;
        }
        if !matches_scope_dimension(&parsed.people, &query.people) {
            continue;
        }
        if !matches_scope_dimension(&parsed.channels, &query.channels) {
            continue;
        }
        if !query.domains.is_empty() && !query.domains.iter().any(|d| parsed.domains.contains(d)) {
            continue;
        }
        if !in_time_window(&parsed.created_at, &query.since, &query.until) {
            continue;
        }
        // Релевантность вычисляется один раз на сущность: и для фильтра, и
        // для итогового элемента (раньше — дважды, с перетокенизацией запроса).
        let relevance = relevance_of_terms(parsed, &terms);
        if !query_text.is_empty() && relevance <= 0 {
            continue;
        }
        items.push(to_context_item(parsed, relevance));
    }
    items.sort_by(compare_items);

    // Упаковка: добавляем в порядке приоритета, пока оценка ПОЛНОГО пакета
    // (заголовок + тело + отчёт о конфликтах/superseded) не превысит бюджет.
    // stateVersion — 8 hex-символов, размер не зависит от значения, поэтому
    // в пробной оценке используется заглушка.
    //
    // SESSION-14: оценка инкрементальная и целочисленная — части заголовка и
    // хвоста (конфликты/superseded) считаются один раз, тело растёт на
    // фиксированную стоимость каждого принятого элемента. Это ровно тот же
    // результат, что `estimate_tokens(вся строка)` (аддитивность + целочисленное
    // деление), но O(n) вместо O(n²) пересборок полной строки.
    let prefix_units = Units::of(&format!(
        "SOUL CONTEXT\npolicy: {CONTEXT_POLICY_VERSION}\nstate: 00000000\ntokens: X of {}\nentities: ",
        format_tokens(max_tokens)
    ));
    let mut tail_units = Units::default();
    if !conflicts.is_empty() {
        let mut block = String::from("CONFLICTS:");
        for c in &conflicts {
            block.push('\n');
            block.push_str(&format!("- {} vs {}: {}", c.a, c.b, c.reason));
        }
        tail_units.add_char('\n');
        tail_units.add_str(&block);
    }
    if !superseded_ids.is_empty() {
        tail_units.add_char('\n');
        tail_units.add_str(&format!("SUPERSEDED: {}", superseded_ids.join(", ")));
    }

    let mut packed: Vec<ContextItem> = Vec::new();
    let mut candidate_texts: Vec<String> = Vec::new();
    let mut body_units = Units::default();
    for item in &items {
        let mut lines = vec![format!(
            "[{}] {} / {} / {}",
            item.id, item.entity_type, item.status, item.sensitivity
        )];
        lines.push(item.claim.clone());
        if !item.evidence.is_empty() && item.evidence != item.claim {
            lines.push(format!("evidence: {}", item.evidence));
        }
        let text = lines.join("\n");
        let text_units = Units::of(&text);
        let trial_count = candidate_texts.len() + 1;
        let count_units = Units::of(&format!("{trial_count}\n"));
        let mut trial_units = prefix_units;
        trial_units.cjk += count_units.cjk;
        trial_units.latin += count_units.latin;
        trial_units.cjk += body_units.cjk;
        trial_units.latin += body_units.latin;
        // Разделители между элементами тела (join("\n") между частями).
        if !candidate_texts.is_empty() {
            trial_units.cjk += SEP_UNITS.cjk * candidate_texts.len() as u64;
            trial_units.latin += SEP_UNITS.latin * candidate_texts.len() as u64;
        }
        trial_units.cjk += text_units.cjk;
        trial_units.latin += text_units.latin;
        trial_units.cjk += tail_units.cjk;
        trial_units.latin += tail_units.latin;
        if trial_units.tokens() <= max_tokens {
            packed.push(item.clone());
            candidate_texts.push(text);
            body_units.cjk += text_units.cjk;
            body_units.latin += text_units.latin;
        }
    }

    // Финальная сборка: версия состояния по включённым сущностям; токены в
    // заголовке — оценка реального сериализованного текста (не заглушки).
    let finalize = |count: usize| -> (Vec<ContextItem>, String, u64, String) {
        let selected = packed[..count].to_vec();
        let body = candidate_texts[..count].join("\n");
        let mut state_source: Vec<String> = selected
            .iter()
            .map(|i| format!("{}|{}", i.id, i.updated_at))
            .collect();
        state_source.sort();
        let state_version = hash_string(&state_source.join("\n"));
        let draft = serialize_pack_body_parts(
            &body,
            &conflicts,
            &superseded_ids,
            &state_version,
            selected.len(),
            max_tokens,
        );
        let token_estimate = estimate_tokens(&draft);
        let serialized = draft.replacen(
            "tokens: X of",
            &format!("tokens: {} of", format_tokens(token_estimate)),
            1,
        );
        (
            selected,
            state_version,
            estimate_tokens(&serialized),
            serialized,
        )
    };

    // Страховка: замена 'X' на число добавляет до пары символов — если оценка
    // финального текста всё же превысила бюджет, сбрасываем самый
    // низкоприоритетный элемент и пересобираем. Детерминированно, максимум пара
    // итераций.
    let mut finalized = finalize(packed.len());
    while finalized.2 > max_tokens && !finalized.0.is_empty() {
        finalized = finalize(finalized.0.len() - 1);
    }

    ContextPack {
        items: finalized.0,
        conflicts,
        superseded_ids,
        policy_version: CONTEXT_POLICY_VERSION.to_string(),
        state_version: finalized.1,
        max_tokens,
        token_estimate: finalized.2,
        serialized: finalized.3,
    }
}

/// Канонический ключ запроса для кеша: полная сериализация всех фильтров +
/// нормализованный бюджет токенов. Поле max_tokens исключается из JSON и
/// заменяется нормализованным целым бюджетом (None и 900 эквивалентны;
/// нестандартные бюджеты отличаются). Одинаковые запросы всегда дают
/// одинаковый ключ; разные — разные.
pub fn query_fingerprint(query: &ContextQuery) -> String {
    let effective = match query.max_tokens {
        Some(t) if t.is_finite() => (t.max(1.0).min(CONTEXT_HARD_MAX_TOKENS as f64)) as u64,
        _ => CONTEXT_STANDARD_TOKENS,
    };
    let mut normalized = query.clone();
    normalized.max_tokens = None;
    let mut json = serde_json::to_string(&normalized).unwrap_or_default();
    json.push('|');
    json.push_str(&effective.to_string());
    json
}

/// Процессный кеш последнего скомпилированного контекста (SESSION-14).
/// Ключ: путь к БД (разные файлы в тестах/процессах не пересекаются) +
/// ревизии состояния и политик + ключ запроса. Ревизии монотонно растут при
/// каждой мутации в той же транзакции (db.rs), поэтому закешированный пак
/// никогда не переживает изменения данных: любой INSERT/UPDATE/DELETE/импорт
/// меняет ревизию ещё до коммита мутации.
struct CachedContext {
    db_path: String,
    state_revision: i64,
    policy_revision: i64,
    fingerprint: String,
    pack: ContextPack,
}

static CONTEXT_CACHE: Mutex<Option<CachedContext>> = Mutex::new(None);

fn cached_context(
    db_path: &str,
    state_revision: i64,
    policy_revision: i64,
    fingerprint: &str,
) -> Option<ContextPack> {
    let guard = CONTEXT_CACHE.lock().ok()?;
    let cached = guard.as_ref()?;
    (cached.db_path == db_path
        && cached.state_revision == state_revision
        && cached.policy_revision == policy_revision
        && cached.fingerprint == fingerprint)
        .then(|| cached.pack.clone())
}

fn cache_context(
    db_path: String,
    state_revision: i64,
    policy_revision: i64,
    fingerprint: String,
    pack: ContextPack,
) {
    if let Ok(mut guard) = CONTEXT_CACHE.lock() {
        *guard = Some(CachedContext {
            db_path,
            state_revision,
            policy_revision,
            fingerprint,
            pack,
        });
    }
}

fn db_file_path(conn: &rusqlite::Connection) -> String {
    conn.query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        .unwrap_or_default()
}

/// Чтение БД (read-only) → компиляция контекста с процессным кешем по
/// версии состояния и политики. Каждый вызов по-прежнему пишет disclosure
/// квитанцию у вызывающего — кеш экономит только чтение сущностей и
/// компиляцию, не аудит.
pub fn compile_context_cached(
    conn: &rusqlite::Connection,
    query: &ContextQuery,
) -> Result<ContextPack, String> {
    let db_path = db_file_path(conn);
    let state_revision = db::state_revision(conn).map_err(|e| e.to_string())?;
    let policy_revision = db::policy_revision(conn).map_err(|e| e.to_string())?;
    let fingerprint = query_fingerprint(query);
    if let Some(pack) = cached_context(&db_path, state_revision, policy_revision, &fingerprint) {
        return Ok(pack);
    }

    let souls = db::list_souls(conn).map_err(|e| format!("Cannot read SOUL database: {e}"))?;
    let entities: Vec<ContextEntity> = match souls.first() {
        Some(soul) => db::list_entities(conn, &soul.soul_id)
            .map_err(|e| format!("Cannot read SOUL database: {e}"))?
            .into_iter()
            .map(|r| ContextEntity {
                id: r.id,
                soul_id: r.soul_id,
                entity_type: r.entity_type,
                status: r.status,
                data: r.data,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect(),
        None => Vec::new(),
    };
    let pack = compile_context(&entities, query);

    cache_context(
        db_path,
        state_revision,
        policy_revision,
        fingerprint,
        pack.clone(),
    );
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(
        id: &str,
        entity_type: &str,
        status: &str,
        data: &str,
        created_at: &str,
        updated_at: &str,
    ) -> ContextEntity {
        ContextEntity {
            id: id.to_string(),
            soul_id: "soul_test".to_string(),
            entity_type: entity_type.to_string(),
            status: status.to_string(),
            data: data.to_string(),
            created_at: created_at.to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    fn calibration_data(
        question_id: &str,
        value: &str,
        sensitivity: &str,
        confidence: f64,
    ) -> String {
        format!(
            r#"{{"claim":"Q — {value}","evidence":"stated","questionId":"{question_id}","value":"{value}","confidence":{confidence},"sensitivity":"{sensitivity}","scope":{{"domains":["preferences"],"projects":[],"people":[],"channels":[]}}}}"#
        )
    }

    fn query(mut over: ContextQuery) -> ContextQuery {
        if over.max_tokens.is_none() {
            over.max_tokens = Some(CONTEXT_STANDARD_TOKENS as f64);
        }
        over
    }

    #[test]
    fn hash_string_matches_standard_fnv1a32_vectors() {
        assert_eq!(hash_string(""), "811c9dc5");
        assert_eq!(hash_string("a"), "e40c292c");
        assert_eq!(hash_string("foobar"), "bf9cf968");
    }

    #[test]
    fn estimate_tokens_counts_cjk_and_latin() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("编程"), 2);
        assert_eq!(estimate_tokens("中文测试"), 4);
        assert_eq!(estimate_tokens("abc"), 1); // 3 * 1/3 = 1.0
        assert_eq!(estimate_tokens("abcd"), 2); // ceil(4/3) = 2
    }

    #[test]
    fn format_tokens_uses_en_us_grouping() {
        assert_eq!(format_tokens(900), "900");
        assert_eq!(format_tokens(1000), "1,000");
        assert_eq!(format_tokens(3000), "3,000");
        assert_eq!(format_tokens(1234567), "1,234,567");
    }

    #[test]
    fn golden_serialization_matches_ts_layout() {
        let a = entity(
            "ent_a",
            "preference",
            "active",
            &calibration_data("pref_speed", "concise", "internal", 0.9),
            "2026-07-01T00:00:00Z",
            "2026-07-10T00:00:00Z",
        );
        let b = entity(
            "ent_b",
            "preference",
            "active",
            &calibration_data("pref_speed", "detailed", "internal", 0.8),
            "2026-05-01T00:00:00Z",
            "2026-06-01T00:00:00Z",
        );
        let c = entity(
            "ent_c",
            "boundary",
            "active",
            &calibration_data("bound_health", "never", "internal", 0.8),
            "2026-06-15T00:00:00Z",
            "2026-07-05T00:00:00Z",
        );

        let pack = compile_context(
            &[a.clone(), b.clone(), c.clone()],
            &query(ContextQuery::default()),
        );
        assert_eq!(pack.max_tokens, 900);
        assert_eq!(pack.items.len(), 2);
        assert_eq!(pack.items[0].entity_type, "boundary");
        assert_eq!(pack.items[1].id, "ent_a");
        assert_eq!(pack.conflicts.len(), 1);
        assert_eq!(pack.conflicts[0].a, "ent_a");
        assert_eq!(pack.conflicts[0].b, "ent_b");
        assert_eq!(pack.superseded_ids, vec!["ent_b".to_string()]);

        // Тот же золотой литерал, что и в TS golden-тесте (tests/context.test.ts):
        // жёстко зафиксированные state-хэш и количество токенов.
        let expected = "SOUL CONTEXT\npolicy: soul-context-policy/1\nstate: 5b38f537\ntokens: 110 of 900\nentities: 2\n[ent_c] boundary / active / internal\nQ — never\nevidence: stated\n[ent_a] preference / active / internal\nQ — concise\nevidence: stated\nCONFLICTS:\n- ent_a vs ent_b: Same calibration question (pref_speed) with different answers\nSUPERSEDED: ent_b";
        assert_eq!(pack.serialized, expected);
        assert_eq!(pack.state_version, "5b38f537");
        // Оценка применяется к реальному сериализованному пакету («110» на 2
        // символа длиннее «X») — этот сдвиг зафиксирован в обоих языках.
        assert_eq!(pack.token_estimate, 110);
        assert_eq!(pack.token_estimate, estimate_tokens(expected));
    }

    #[test]
    fn determinism_input_order_does_not_matter() {
        let a = entity(
            "ent_a",
            "preference",
            "active",
            &calibration_data("pref_order", "x", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-06-01T00:00:00Z",
        );
        let b = entity(
            "ent_b",
            "preference",
            "active",
            &calibration_data("pref_order", "y", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-07-01T00:00:00Z",
        );
        let c = entity(
            "ent_c",
            "boundary",
            "active",
            &calibration_data("bound_x", "never", "sensitive", 0.8),
            "2026-01-01T00:00:00Z",
            "2026-07-05T00:00:00Z",
        );
        let forward = vec![a.clone(), b.clone(), c.clone()];
        let mut backward = forward.clone();
        backward.reverse();
        let p1 = compile_context(&forward, &query(ContextQuery::default()));
        let p2 = compile_context(&backward, &query(ContextQuery::default()));
        assert_eq!(p1.serialized, p2.serialized);
        assert_eq!(p1.state_version, p2.state_version);
        assert_eq!(p1.conflicts, p2.conflicts);
        assert_eq!(p1.superseded_ids, p2.superseded_ids);
    }

    #[test]
    fn state_version_changes_when_content_changes() {
        let a = entity(
            "ent_a",
            "preference",
            "active",
            &calibration_data("pref_1", "one", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let b = entity(
            "ent_b",
            "preference",
            "active",
            &calibration_data("pref_1", "two", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-02-01T00:00:00Z",
        );
        let p1 = compile_context(&[a], &query(ContextQuery::default()));
        let p2 = compile_context(&[b], &query(ContextQuery::default()));
        assert_ne!(p1.state_version, p2.state_version);
    }

    #[test]
    fn only_active_by_default_and_explicit_statuses_are_allowed() {
        let active = entity(
            "ent_a",
            "preference",
            "active",
            &calibration_data("pref_1", "x", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let disputed = entity(
            "ent_b",
            "preference",
            "disputed",
            &calibration_data("pref_2", "x", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let pack = compile_context(
            &[active.clone(), disputed.clone()],
            &query(ContextQuery::default()),
        );
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].id, "ent_a");

        let pack = compile_context(
            &[disputed],
            &query(ContextQuery {
                statuses: vec!["disputed".to_string()],
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
    }

    #[test]
    fn restricted_sensitivity_excluded_by_default_and_included_when_asked() {
        let restricted = entity(
            "ent_a",
            "fact",
            "active",
            &calibration_data("text_secret", "x", "restricted", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let pack = compile_context(
            std::slice::from_ref(&restricted),
            &query(ContextQuery::default()),
        );
        assert!(pack.items.is_empty());

        let pack = compile_context(
            &[restricted],
            &query(ContextQuery {
                sensitivity: vec!["restricted".to_string()],
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
    }

    #[test]
    fn scope_and_domain_filters() {
        let inside = entity(
            "ent_a",
            "preference",
            "active",
            r#"{"claim":"x","questionId":"goal_x","value":"x","confidence":0.9,"sensitivity":"internal","scope":{"domains":["goals"],"projects":["SOUL"],"people":[],"channels":[]}}"#,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let outside = entity(
            "ent_b",
            "preference",
            "active",
            r#"{"claim":"y","questionId":"dec_1","value":"y","confidence":0.9,"sensitivity":"internal","scope":{"domains":["decisions"],"projects":["NIMBUS"],"people":[],"channels":[]}}"#,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        // Фильтр по проекту: совпадение по scope.projects.
        let pack = compile_context(
            &[inside.clone(), outside.clone()],
            &query(ContextQuery {
                projects: vec!["SOUL".to_string()],
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].id, "ent_a");

        // Фильтр по домену: goal_x -> goals (ent_a), dec_1 -> decisions (ent_b).
        let pack = compile_context(
            &[inside, outside],
            &query(ContextQuery {
                domains: vec!["decisions".to_string()],
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].id, "ent_b");
    }

    #[test]
    fn time_window_filters_and_ignores_unparseable() {
        let inside = entity(
            "ent_a",
            "preference",
            "active",
            &calibration_data("pref_1", "x", "internal", 0.9),
            "2026-07-15T00:00:00Z",
            "2026-07-15T00:00:00Z",
        );
        let too_old = entity(
            "ent_b",
            "preference",
            "active",
            &calibration_data("pref_2", "x", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let pack = compile_context(
            &[inside.clone(), too_old],
            &query(ContextQuery {
                since: Some("2026-07-01T00:00:00Z".to_string()),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].id, "ent_a");

        let weird = entity(
            "ent_w",
            "preference",
            "active",
            &calibration_data("pref_3", "x", "internal", 0.9),
            "not-a-date",
            "not-a-date",
        );
        let pack = compile_context(
            &[weird],
            &query(ContextQuery {
                since: Some("2026-07-01T00:00:00Z".to_string()),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
    }

    #[test]
    fn text_query_filters_by_relevance() {
        let match_claim = entity(
            "ent_a",
            "preference",
            "active",
            r#"{"claim":"Prefers concise answers","questionId":"pref_1","value":"x","confidence":0.9,"sensitivity":"internal","scope":{"domains":[],"projects":[],"people":[],"channels":[]}}"#,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let no_match = entity(
            "ent_b",
            "preference",
            "active",
            r#"{"claim":"Likes long walks","questionId":"pref_2","value":"x","confidence":0.9,"sensitivity":"internal","scope":{"domains":[],"projects":[],"people":[],"channels":[]}}"#,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let pack = compile_context(
            &[match_claim, no_match],
            &query(ContextQuery {
                text: "concise".to_string(),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].id, "ent_a");
    }

    #[test]
    fn relevance_dedupes_query_terms_like_ts_set() {
        // TS токенизирует запрос в Set — повтор слова не удваивает счёт.
        // Rust обязан давать тот же счёт (и тот же порядок сортировки).
        let match_claim = entity(
            "ent_a",
            "preference",
            "active",
            r#"{"claim":"Prefers concise answers","questionId":"pref_1","value":"x","confidence":0.9,"sensitivity":"internal","scope":{"domains":[],"projects":[],"people":[],"channels":[]}}"#,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );
        let single = relevance_of(&match_claim, "concise");
        let repeated = relevance_of(&match_claim, "concise concise");
        assert_eq!(single, repeated);
        assert_eq!(single, 2, "термин в claim = ×2");
    }

    #[test]
    fn budget_never_exceeds_soft_or_hard_limits() {
        let mut entities: Vec<ContextEntity> = Vec::new();
        for i in 0..40 {
            entities.push(entity(
                &format!("ent_{i:02}"),
                "preference",
                "active",
                &format!(
                    r#"{{"claim":"Long preference number {i}: {}","questionId":"pref_long_{i}","value":"x","confidence":0.9,"sensitivity":"internal","scope":{{"domains":[],"projects":[],"people":[],"channels":[]}}}}"#,
                    "padding ".repeat(60).trim_end()
                ),
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ));
        }
        let soft = compile_context(
            &entities,
            &query(ContextQuery {
                max_tokens: Some(CONTEXT_STANDARD_TOKENS as f64),
                ..ContextQuery::default()
            }),
        );
        assert!(soft.token_estimate <= CONTEXT_STANDARD_TOKENS);
        assert!(!soft.items.is_empty());
        assert!(soft.items.len() < 40);

        let hard = compile_context(
            &entities,
            &query(ContextQuery {
                max_tokens: Some(CONTEXT_HARD_MAX_TOKENS as f64),
                ..ContextQuery::default()
            }),
        );
        assert!(hard.token_estimate <= CONTEXT_HARD_MAX_TOKENS);

        let huge = compile_context(
            &entities,
            &query(ContextQuery {
                max_tokens: Some(99_999.0),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(huge.max_tokens, CONTEXT_HARD_MAX_TOKENS);

        let tiny = compile_context(
            &entities,
            &query(ContextQuery {
                max_tokens: Some(0.0),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(tiny.max_tokens, 1);
    }

    #[test]
    fn empty_pack_is_still_serialized_within_budget() {
        let pack = compile_context(&[], &query(ContextQuery::default()));
        assert!(pack.items.is_empty());
        assert!(pack.serialized.contains("SOUL CONTEXT"));
        assert!(pack.token_estimate <= 30);
    }

    #[test]
    fn header_conflicts_and_superseded_count_toward_budget() {
        let old_answer = entity(
            "ent_old",
            "preference",
            "active",
            &calibration_data("pref_boundary", "fast", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-06-01T00:00:00Z",
        );
        let new_answer = entity(
            "ent_new",
            "preference",
            "active",
            &calibration_data("pref_boundary", "slow", "internal", 0.9),
            "2026-01-01T00:00:00Z",
            "2026-07-01T00:00:00Z",
        );
        let mut others: Vec<ContextEntity> = Vec::new();
        for i in 0..10 {
            others.push(entity(
                &format!("ent_extra_{i}"),
                "preference",
                "active",
                &format!(
                    r#"{{"claim":"Padding {}{}","questionId":"pref_extra_{i}","value":"x","confidence":0.9,"sensitivity":"internal","scope":{{"domains":[],"projects":[],"people":[],"channels":[]}}}}"#,
                    "pad ".repeat(30),
                    i
                ),
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ));
        }
        let mut all = vec![old_answer, new_answer];
        all.extend(others);
        let pack = compile_context(
            &all,
            &query(ContextQuery {
                max_tokens: Some(250.0),
                ..ContextQuery::default()
            }),
        );
        assert_eq!(pack.conflicts.len(), 1);
        assert!(pack.superseded_ids.contains(&"ent_old".to_string()));
        assert!(pack.serialized.contains("CONFLICTS:"));
        assert!(pack.token_estimate <= pack.max_tokens);
    }

    #[test]
    fn cost_estimate_is_conservative_and_stable() {
        assert_eq!(cost_estimate_usd(0), 0.0);
        assert_eq!(cost_estimate_usd(1000), 0.005);
        assert!((cost_estimate_usd(900) - 0.0045).abs() < 1e-12);
        assert!((cost_estimate_usd(2000) - 0.01).abs() < 1e-12);
        assert!((cost_estimate_usd(3000) - 0.015).abs() < 1e-12);
    }

    #[test]
    fn query_fingerprint_is_stable_and_distinct() {
        let base = ContextQuery::default();
        assert_eq!(query_fingerprint(&base), query_fingerprint(&base));
        // None и стандартный бюджет эквивалентны.
        let with_std = ContextQuery {
            max_tokens: Some(CONTEXT_STANDARD_TOKENS as f64),
            ..ContextQuery::default()
        };
        assert_eq!(query_fingerprint(&base), query_fingerprint(&with_std));
        // Разные тексты — разные ключи.
        let with_text = ContextQuery {
            text: "другое".to_string(),
            ..ContextQuery::default()
        };
        assert_ne!(query_fingerprint(&base), query_fingerprint(&with_text));
        // Нестандартные бюджеты различаются.
        let with_big = ContextQuery {
            max_tokens: Some(2000.0),
            ..ContextQuery::default()
        };
        assert_ne!(query_fingerprint(&base), query_fingerprint(&with_big));
    }

    #[test]
    fn compile_context_cached_returns_same_pack_until_mutation() {
        let dir =
            std::env::temp_dir().join(format!("soul-ctx-cache-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::init_db(&dir).unwrap();
        let soul = crate::db::create_soul(&conn, "Тест", "device_c").unwrap();
        crate::db::add_entity(
            &conn,
            &soul.soul_id,
            "preference",
            "active",
            r#"{"claim":"Prefers concise answers","questionId":"pref_1","value":"concise","confidence":0.9,"sensitivity":"internal","scope":{"domains":["preferences"],"projects":[],"people":[],"channels":[]}}"#,
            "device_c",
        )
        .unwrap();

        let q = query(ContextQuery::default());
        let first = compile_context_cached(&conn, &q).unwrap();
        let second = compile_context_cached(&conn, &q).unwrap();
        assert_eq!(first.serialized, second.serialized);
        assert_eq!(first.state_version, second.state_version);

        // Мутация (add_entity) повышает ревизию состояния в той же транзакции —
        // кеш обязан пересобрать пак с новым состоянием.
        crate::db::add_entity(
            &conn,
            &soul.soul_id,
            "preference",
            "active",
            r#"{"claim":"Prefers bullet points","questionId":"pref_2","value":"bullets","confidence":0.9,"sensitivity":"internal","scope":{"domains":["preferences"],"projects":[],"people":[],"channels":[]}}"#,
            "device_c",
        )
        .unwrap();
        let third = compile_context_cached(&conn, &q).unwrap();
        assert_ne!(
            first.state_version, third.state_version,
            "cache must invalidate on state mutation"
        );
        assert!(third.serialized.contains("bullet points"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compile_context_over_ten_thousand_entities_is_fast() {
        let mut all: Vec<ContextEntity> = Vec::new();
        for i in 0..10_000 {
            all.push(entity(
                &format!("ent_{i}"),
                "preference",
                "active",
                &format!(
                    r#"{{"claim":"Memory item {i} about topic_{}","evidence":"notes","source":"manual","questionId":"q_{i}","value":"v{i}","confidence":0.5,"sensitivity":"internal","scope":{{"domains":["memory"],"projects":[],"people":[],"channels":[]}}}}"#,
                    i % 50
                ),
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ));
        }

        let start = std::time::Instant::now();
        let pack = compile_context(&all, &query(ContextQuery::default()));
        let elapsed = start.elapsed();
        assert!(pack.token_estimate <= pack.max_tokens, "budget must hold");
        // Порог для debug-сборки с учётом параллельного прогона тестов: ловит
        // квадратичные регрессии (на 10k O(n²) даст десятки секунд), а не
        // константные накладные. Реальные p95 из release — в SESSION-14.md.
        assert!(
            elapsed.as_millis() < 5000,
            "compile over 10k entities took {:?}",
            elapsed
        );
    }

    #[test]
    #[ignore = "run through pnpm release:check; measures the production p95 budget"]
    fn release_context_p95_is_under_75ms_for_one_thousand_entities() {
        let mut all: Vec<ContextEntity> = Vec::new();
        for i in 0..1_000 {
            all.push(entity(
                &format!("perf_{i}"),
                "preference",
                "active",
                &format!(
                    r#"{{"claim":"Performance memory {i} about topic_{}","evidence":"notes","questionId":"perf_{i}","value":"v{i}","confidence":0.5,"sensitivity":"internal","scope":{{"domains":["memory"],"projects":[],"people":[],"channels":[]}}}}"#,
                    i % 50
                ),
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ));
        }
        let q = query(ContextQuery {
            text: "topic 7".to_string(),
            ..ContextQuery::default()
        });
        let _ = compile_context(&all, &q); // warmup

        let mut samples: Vec<std::time::Duration> = Vec::new();
        for _ in 0..25 {
            let start = std::time::Instant::now();
            let pack = compile_context(&all, &q);
            assert!(pack.token_estimate <= pack.max_tokens);
            samples.push(start.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
        eprintln!("release cold context p95 over 1k entities: {p95:?}");
        assert!(
            p95 < std::time::Duration::from_millis(75),
            "release context p95 exceeded 75ms: {p95:?}"
        );
    }

    #[test]
    #[ignore = "run through pnpm release:check; measures the cached production path"]
    fn release_cached_context_p95_is_under_75ms_for_ten_thousand_entities() {
        let mut all: Vec<ContextEntity> = Vec::new();
        for i in 0..10_000 {
            all.push(entity(
                &format!("cache_{i}"),
                "preference",
                "active",
                &format!(
                    r#"{{"claim":"Cached performance memory {i} about topic_{}","questionId":"cache_{i}","value":"v{i}","confidence":0.5,"sensitivity":"internal","scope":{{"domains":["memory"],"projects":[],"people":[],"channels":[]}}}}"#,
                    i % 50
                ),
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ));
        }
        let q = query(ContextQuery {
            text: "topic 7".to_string(),
            ..ContextQuery::default()
        });
        let fingerprint = query_fingerprint(&q);
        let cache_key = format!("release-cache-benchmark-{}", uuid::Uuid::new_v4());
        let pack = compile_context(&all, &q);
        cache_context(cache_key.clone(), 1, 1, fingerprint.clone(), pack);

        let mut samples: Vec<std::time::Duration> = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let pack = cached_context(&cache_key, 1, 1, &fingerprint).expect("cache hit");
            assert!(pack.token_estimate <= pack.max_tokens);
            samples.push(start.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
        eprintln!("release cached context p95 over 10k entities: {p95:?}");
        assert!(
            p95 < std::time::Duration::from_millis(75),
            "release cached context p95 exceeded 75ms: {p95:?}"
        );
    }
}
