//! Детерминированный компилятор контекста (Rust-порт SESSION-07 TS-модуля).
//!
//! Чистые функции: без сети, без модели, без часов и случайности — одинаковое
//! состояние SOUL + одинаковый запрос всегда дают одинаковый пак. Это
//! единственный источник истины для MCP-сервера (`soul.get_context`); TS-копия
//! используется только в UI и держится синхронной (golden-тесты по обеим
//! сторонам). Семантика и сериализация совпадают с `src/data/context.ts`.

use serde::{Deserialize, Serialize};

pub const CONTEXT_POLICY_VERSION: &str = "soul-context-policy/1";
pub const CONTEXT_STANDARD_TOKENS: u64 = 900;
pub const CONTEXT_HARD_MAX_TOKENS: u64 = 3000;

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

#[derive(Debug, Clone, Default, Deserialize)]
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

fn obj_of(data: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    parse_data(data).as_object().cloned()
}

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

fn sensitivity_of(entity: &ContextEntity) -> String {
    let value = str_of(&entity.data, "sensitivity").unwrap_or_default();
    if ALL_SENSITIVITY.contains(&value.as_str()) {
        value
    } else {
        "internal".to_string()
    }
}

fn domains_of(entity: &ContextEntity) -> Vec<String> {
    let data = parse_data(&entity.data);
    if let Some(scope) = data.get("scope").and_then(|s| s.as_object()) {
        if let Some(list) = scope.get("domains").and_then(|d| d.as_array()) {
            let strings: Vec<String> = list
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !strings.is_empty() {
                return strings;
            }
        }
    }
    if let Some(q) = str_of(&entity.data, "questionId") {
        if !q.is_empty() {
            return vec![domain_for_question(&q).to_string()];
        }
    }
    Vec::new()
}

fn question_id_of(entity: &ContextEntity) -> Option<String> {
    let q = str_of(&entity.data, "questionId")?;
    if q.is_empty() {
        None
    } else {
        Some(q)
    }
}

fn value_of(entity: &ContextEntity) -> serde_json::Value {
    parse_data(&entity.data)
        .get("value")
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

fn confidence_of(entity: &ContextEntity) -> f64 {
    let Some(c) = obj_of(&entity.data)
        .and_then(|o| o.get("confidence").cloned())
        .and_then(|v| v.as_f64())
    else {
        return 0.0;
    };
    if c.is_finite() {
        c.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn scope_dim(entity: &ContextEntity, dim: &str) -> Vec<String> {
    let data = parse_data(&entity.data);
    let Some(list) = data
        .get("scope")
        .and_then(|s| s.as_object())
        .and_then(|o| o.get(dim))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
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

pub struct DedupResult {
    pub kept: Vec<ContextEntity>,
    pub superseded_ids: Vec<String>,
}

/// Удаление замещённых дубликатов: из сущностей с одинаковым questionId
/// (перекомпиляция калибровки) остаётся только самая свежая по updated_at.
/// Сущности без questionId (legacy) не трогаются. Старые ответы помечаются
/// в superseded_ids — они не попадают в пак, но видны в отчёте.
pub fn dedupe_superseded(entities: &[ContextEntity]) -> DedupResult {
    let mut by_question: Vec<(String, Vec<ContextEntity>)> = Vec::new();
    let mut standalone: Vec<ContextEntity> = Vec::new();
    for entity in entities {
        match question_id_of(entity) {
            None => standalone.push(entity.clone()),
            Some(q) => {
                if let Some((_, list)) = by_question.iter_mut().find(|(k, _)| *k == q) {
                    list.push(entity.clone());
                } else {
                    by_question.push((q, vec![entity.clone()]));
                }
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
        let newest = group[0].clone();
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
pub fn detect_conflicts(entities: &[ContextEntity]) -> Vec<ContextConflict> {
    let mut by_question: Vec<(String, Vec<ContextEntity>)> = Vec::new();
    for entity in entities {
        let Some(q) = question_id_of(entity) else {
            continue;
        };
        if let Some((_, list)) = by_question.iter_mut().find(|(k, _)| *k == q) {
            list.push(entity.clone());
        } else {
            by_question.push((q, vec![entity.clone()]));
        }
    }
    let mut conflicts: Vec<ContextConflict> = Vec::new();
    for (question_id, group) in by_question {
        let mut by_value: Vec<(String, Vec<ContextEntity>)> = Vec::new();
        for entity in &group {
            let key = serde_json::to_string(&value_of(entity)).unwrap_or_default();
            if let Some((_, list)) = by_value.iter_mut().find(|(k, _)| *k == key) {
                list.push(entity.clone());
            } else {
                by_value.push((key, vec![entity.clone()]));
            }
        }
        if by_value.len() < 2 {
            continue;
        }
        let mut representatives: Vec<ContextEntity> = Vec::new();
        for (_, group) in by_value {
            representatives.push(group[0].clone());
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

fn to_context_item(entity: &ContextEntity, query_text: &str) -> ContextItem {
    let claim = str_of(&entity.data, "claim").unwrap_or_default();
    ContextItem {
        id: entity.id.clone(),
        entity_type: entity.entity_type.clone(),
        status: entity.status.clone(),
        claim,
        evidence: str_of(&entity.data, "evidence").unwrap_or_default(),
        sensitivity: sensitivity_of(entity),
        domains: domains_of(entity),
        relevance: relevance_of(entity, query_text),
        priority: priority_tier(&entity.entity_type),
        confidence: confidence_of(entity),
        updated_at: entity.updated_at.clone(),
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

    // Конфликты считаются по сырым данным: повторные ответы на один вопрос с
    // другим значением дают пару (старое значение → новое) ещё до дедупликации.
    let mut conflicts = detect_conflicts(entities);
    conflicts.sort_by(|x, y| format!("{}|{}", x.a, x.b).cmp(&format!("{}|{}", y.a, y.b)));

    let dedup = dedupe_superseded(entities);
    let mut superseded_ids = dedup.superseded_ids;
    superseded_ids.sort();

    let eligible: Vec<&ContextEntity> = dedup
        .kept
        .iter()
        .filter(|entity| {
            if !allowed_statuses.iter().any(|s| s == &entity.status) {
                return false;
            }
            if !allowed_sensitivity
                .iter()
                .any(|s| *s == sensitivity_of(entity))
            {
                return false;
            }
            if !matches_scope_dimension(&scope_dim(entity, "projects"), &query.projects) {
                return false;
            }
            if !matches_scope_dimension(&scope_dim(entity, "people"), &query.people) {
                return false;
            }
            if !matches_scope_dimension(&scope_dim(entity, "channels"), &query.channels) {
                return false;
            }
            let entity_domains = domains_of(entity);
            if !query.domains.is_empty()
                && !query.domains.iter().any(|d| entity_domains.contains(d))
            {
                return false;
            }
            if !in_time_window(&entity.created_at, &query.since, &query.until) {
                return false;
            }
            if !query_text.is_empty() && relevance_of(entity, &query_text) <= 0 {
                return false;
            }
            true
        })
        .collect();

    let mut items: Vec<ContextItem> = eligible
        .iter()
        .map(|e| to_context_item(e, &query_text))
        .collect();
    items.sort_by(compare_items);

    // Упаковка: добавляем в порядке приоритета, пока оценка ПОЛНОГО пакета
    // (заголовок + тело + отчёт о конфликтах/superseded) не превысит бюджет.
    // stateVersion — 8 hex-символов, размер не зависит от значения, поэтому
    // в пробной оценке используется заглушка.
    let mut packed: Vec<ContextItem> = Vec::new();
    let mut candidate_texts: Vec<String> = Vec::new();
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
        let mut trial_body = candidate_texts.join("\n");
        if !trial_body.is_empty() {
            trial_body.push('\n');
        }
        trial_body.push_str(&text);
        let trial = serialize_pack_body_parts(
            &trial_body,
            &conflicts,
            &superseded_ids,
            "00000000",
            candidate_texts.len() + 1,
            max_tokens,
        );
        if estimate_tokens(&trial) <= max_tokens {
            packed.push(item.clone());
            candidate_texts.push(text);
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
}
