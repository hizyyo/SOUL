//! Хранилище раундов Blind Preference Test (SESSION-10).
//!
//! Протокол слепого теста требует: (1) вариант с SOUL-контекстом и базовый
//! вариант приходят от пользователя (ручная генерация в его AI-клиенте),
//! (2) назначение слота — какой из ответов (A/B) является SOUL-вариантом —
//! решает host случайно и ЗАПОМИНАЕТ слот до выбора, (3) выбор пользователя
//! фиксируется до раскрытия. Слот назначается здесь (Rust), а не во
//! фронтенде: раскрытие и статистика считаются от сохранённого слота,
//! поэтому после выбора результат не может «подстроиться».

use chrono::Utc;
use rand::Rng;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Максимальная длина ответа варианта (символов) — защита от злоупотребления
/// размером локальной БД.
pub const MAX_VARIANT_ANSWER_CHARS: usize = 20_000;
/// Максимальная длина текста сценария.
pub const MAX_SCENARIO_CHARS: usize = 4_000;
/// Максимальная длина базового профиля (B1).
pub const MAX_BASELINE_PROFILE_CHARS: usize = 4_000;
/// Максимальная длина сериализованного пакета SOUL-контекста.
pub const MAX_CONTEXT_PACK_CHARS: usize = 60_000;
/// Максимальное число id сущностей в паке (защита списка).
pub const MAX_CONTEXT_ENTITY_IDS: usize = 1_000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvaluationRow {
    pub id: String,
    pub soul_id: String,
    pub scenario_id: String,
    pub scenario_text: String,
    pub domain: String,
    /// В каком слоте лежит SOUL-ответ: 'a' или 'b' (случайно при создании).
    pub soul_variant: String,
    pub soul_answer: String,
    pub baseline_answer: String,
    pub baseline_profile: String,
    pub context_pack: String,
    pub context_entity_ids: Vec<String>,
    pub user_choice: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

pub fn init_evaluations(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evaluations (
            id TEXT PRIMARY KEY,
            soul_id TEXT NOT NULL,
            scenario_id TEXT NOT NULL,
            scenario_text TEXT NOT NULL,
            domain TEXT NOT NULL DEFAULT '',
            soul_variant TEXT NOT NULL CHECK (soul_variant IN ('a','b')),
            soul_answer TEXT NOT NULL,
            baseline_answer TEXT NOT NULL,
            baseline_profile TEXT NOT NULL DEFAULT '',
            context_pack TEXT NOT NULL DEFAULT '',
            context_entity_ids TEXT NOT NULL DEFAULT '[]',
            user_choice TEXT CHECK (user_choice IS NULL OR user_choice IN ('a','b','neither')),
            completed_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(soul_id)
        );

        CREATE INDEX IF NOT EXISTS idx_evaluations_soul ON evaluations(soul_id);",
    )?;
    Ok(())
}

fn pick_slot(rng: &mut impl Rng) -> &'static str {
    if rng.gen_bool(0.5) {
        "a"
    } else {
        "b"
    }
}

#[allow(clippy::too_many_arguments)] // поля EvaluationRow переносятся как есть
fn row_from(
    id: &str,
    soul_id: &str,
    scenario_id: &str,
    scenario_text: &str,
    domain: &str,
    soul_variant: &str,
    soul_answer: &str,
    baseline_answer: &str,
    baseline_profile: &str,
    context_pack: &str,
    context_entity_ids: &[String],
) -> EvaluationRow {
    EvaluationRow {
        id: id.to_string(),
        soul_id: soul_id.to_string(),
        scenario_id: scenario_id.to_string(),
        scenario_text: scenario_text.to_string(),
        domain: domain.to_string(),
        soul_variant: soul_variant.to_string(),
        soul_answer: soul_answer.to_string(),
        baseline_answer: baseline_answer.to_string(),
        baseline_profile: baseline_profile.to_string(),
        context_pack: context_pack.to_string(),
        context_entity_ids: context_entity_ids.to_vec(),
        user_choice: None,
        completed_at: None,
        created_at: Utc::now().to_rfc3339(),
    }
}

fn validate_create(
    conn: &Connection,
    soul_id: &str,
    scenario_id: &str,
    scenario_text: &str,
    soul_answer: &str,
    baseline_answer: &str,
    context_entity_ids: &[String],
) -> Result<(), String> {
    if soul_id.trim().is_empty() {
        return Err("soul_id must not be empty.".to_string());
    }
    let soul_exists = conn
        .query_row(
            "SELECT 1 FROM souls WHERE soul_id = ?1",
            params![soul_id],
            |_| Ok(()),
        )
        .is_ok();
    if !soul_exists {
        return Err("SOUL not found.".to_string());
    }
    if scenario_id.trim().is_empty() || scenario_text.trim().is_empty() {
        return Err("scenario_id and scenario_text must not be empty.".to_string());
    }
    if scenario_text.chars().count() > MAX_SCENARIO_CHARS {
        return Err(format!(
            "scenario_text exceeds {} characters.",
            MAX_SCENARIO_CHARS
        ));
    }
    if soul_answer.trim().is_empty() || baseline_answer.trim().is_empty() {
        return Err("Both variant answers must not be empty.".to_string());
    }
    for (name, text) in [
        ("soul_answer", soul_answer),
        ("baseline_answer", baseline_answer),
    ] {
        if text.chars().count() > MAX_VARIANT_ANSWER_CHARS {
            return Err(format!(
                "{name} exceeds {} characters.",
                MAX_VARIANT_ANSWER_CHARS
            ));
        }
    }
    if context_entity_ids.len() > MAX_CONTEXT_ENTITY_IDS {
        return Err("Too many context entity ids.".to_string());
    }
    Ok(())
}

/// Создаёт раунд: слот SOUL-варианта назначается случайно. Проверка протокола —
/// длины, непустота, существование души — до записи.
#[allow(clippy::too_many_arguments)] // параметры = поля EvaluationRow, дублируются в Tauri-команде
pub fn create_evaluation(
    conn: &Connection,
    soul_id: &str,
    scenario_id: &str,
    scenario_text: &str,
    domain: &str,
    soul_answer: &str,
    baseline_answer: &str,
    baseline_profile: &str,
    context_pack: &str,
    context_entity_ids: &[String],
) -> Result<EvaluationRow, String> {
    validate_create(
        conn,
        soul_id,
        scenario_id,
        scenario_text,
        soul_answer,
        baseline_answer,
        context_entity_ids,
    )?;
    if baseline_profile.chars().count() > MAX_BASELINE_PROFILE_CHARS {
        return Err(format!(
            "baseline_profile exceeds {} characters.",
            MAX_BASELINE_PROFILE_CHARS
        ));
    }
    if context_pack.chars().count() > MAX_CONTEXT_PACK_CHARS {
        return Err(format!(
            "context_pack exceeds {} characters.",
            MAX_CONTEXT_PACK_CHARS
        ));
    }
    let id = format!("evl_{}", Uuid::new_v4());
    let slot = pick_slot(&mut rand::thread_rng());
    let row = row_from(
        &id,
        soul_id,
        scenario_id,
        scenario_text,
        domain,
        slot,
        soul_answer,
        baseline_answer,
        baseline_profile,
        context_pack,
        context_entity_ids,
    );
    let ids_json = serde_json::to_string(context_entity_ids)
        .map_err(|e| format!("entity ids serialization failed: {e}"))?;
    conn.execute(
        "INSERT INTO evaluations (
            id, soul_id, scenario_id, scenario_text, domain, soul_variant,
            soul_answer, baseline_answer, baseline_profile, context_pack,
            context_entity_ids, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            row.id,
            row.soul_id,
            row.scenario_id,
            row.scenario_text,
            row.domain,
            row.soul_variant,
            row.soul_answer,
            row.baseline_answer,
            row.baseline_profile,
            row.context_pack,
            ids_json,
            row.created_at
        ],
    )
    .map_err(|e| format!("evaluation insert failed: {e}"))?;
    Ok(row)
}

/// Внутренний вариант с фиксированным слотом — для тестов детерминизма.
#[cfg(test)]
#[allow(clippy::too_many_arguments)] // тестовый дубликат create_evaluation с фиксированным слотом
fn create_evaluation_with_slot(
    conn: &Connection,
    soul_id: &str,
    scenario_id: &str,
    scenario_text: &str,
    domain: &str,
    soul_answer: &str,
    baseline_answer: &str,
    slot: &str,
) -> EvaluationRow {
    let row = row_from(
        &format!("evl_test_{}", Uuid::new_v4()),
        soul_id,
        scenario_id,
        scenario_text,
        domain,
        slot,
        soul_answer,
        baseline_answer,
        "",
        "",
        &[],
    );
    let ids_json = serde_json::to_string(&row.context_entity_ids).unwrap();
    conn.execute(
        "INSERT INTO evaluations (
            id, soul_id, scenario_id, scenario_text, domain, soul_variant,
            soul_answer, baseline_answer, baseline_profile, context_pack,
            context_entity_ids, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            row.id,
            row.soul_id,
            row.scenario_id,
            row.scenario_text,
            row.domain,
            row.soul_variant,
            row.soul_answer,
            row.baseline_answer,
            row.baseline_profile,
            row.context_pack,
            ids_json,
            row.created_at
        ],
    )
    .unwrap();
    row
}

/// Фиксирует выбор пользователя ('a' | 'b' | 'neither'). Повторная подача
/// невозможна: раунд завершается один раз, слот сохранён — раскрытие честное.
pub fn submit_choice(
    conn: &Connection,
    evaluation_id: &str,
    choice: &str,
) -> Result<EvaluationRow, String> {
    if !["a", "b", "neither"].contains(&choice) {
        return Err("choice must be 'a', 'b' or 'neither'.".to_string());
    }
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evaluations WHERE id = ?1",
            params![evaluation_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("evaluation lookup failed: {e}"))?;
    if exists == 0 {
        return Err("Evaluation not found.".to_string());
    }
    let current: Option<String> = conn
        .query_row(
            "SELECT user_choice FROM evaluations WHERE id = ?1",
            params![evaluation_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("evaluation lookup failed: {e}"))?;
    if current.is_some() {
        return Err("Evaluation already completed; choice is final.".to_string());
    }
    conn.execute(
        "UPDATE evaluations SET user_choice = ?1, completed_at = ?2 WHERE id = ?3",
        params![choice, Utc::now().to_rfc3339(), evaluation_id],
    )
    .map_err(|e| format!("evaluation update failed: {e}"))?;
    get_evaluation(conn, evaluation_id).map_err(|e| format!("reload failed: {e}"))
}

fn get_evaluation(conn: &Connection, evaluation_id: &str) -> Result<EvaluationRow, String> {
    conn.query_row(
        "SELECT id, soul_id, scenario_id, scenario_text, domain, soul_variant,
                soul_answer, baseline_answer, baseline_profile, context_pack,
                context_entity_ids, user_choice, completed_at, created_at
         FROM evaluations WHERE id = ?1",
        params![evaluation_id],
        row_from_sql,
    )
    .map_err(|e| format!("evaluation lookup failed: {e}"))
}

fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvaluationRow> {
    let ids_json: String = row.get(10)?;
    let ids: Vec<String> = serde_json::from_str(&ids_json).unwrap_or_default();
    Ok(EvaluationRow {
        id: row.get(0)?,
        soul_id: row.get(1)?,
        scenario_id: row.get(2)?,
        scenario_text: row.get(3)?,
        domain: row.get(4)?,
        soul_variant: row.get(5)?,
        soul_answer: row.get(6)?,
        baseline_answer: row.get(7)?,
        baseline_profile: row.get(8)?,
        context_pack: row.get(9)?,
        context_entity_ids: ids,
        user_choice: row.get(11)?,
        completed_at: row.get(12)?,
        created_at: row.get(13)?,
    })
}

/// Все раунды души, свежие первыми.
pub fn list_evaluations(conn: &Connection, soul_id: &str) -> Result<Vec<EvaluationRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, soul_id, scenario_id, scenario_text, domain, soul_variant,
                    soul_answer, baseline_answer, baseline_profile, context_pack,
                    context_entity_ids, user_choice, completed_at, created_at
             FROM evaluations WHERE soul_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| format!("evaluation list prepare failed: {e}"))?;
    let rows = stmt
        .query_map(params![soul_id], row_from_sql)
        .map_err(|e| format!("evaluation list query failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("evaluation row failed: {e}"))?);
    }
    Ok(out)
}

pub fn delete_evaluation(conn: &Connection, evaluation_id: &str) -> Result<(), String> {
    let n = conn
        .execute(
            "DELETE FROM evaluations WHERE id = ?1",
            params![evaluation_id],
        )
        .map_err(|e| format!("evaluation delete failed: {e}"))?;
    if n == 0 {
        return Err("Evaluation not found.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_soul, init_db};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    struct TestEnv {
        dir: std::path::PathBuf,
        conn: Connection,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-eval-test-{}", Uuid::new_v4()));
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

    fn seed_soul(env: &TestEnv) -> String {
        create_soul(&env.conn, "Тест", "device_e").unwrap().soul_id
    }

    fn base_args(soul_id: &str) -> (&str, &str, &str, &str, &str, &str) {
        (
            soul_id,
            "scen_1",
            "Which job offer would you take?",
            "career",
            "Take the startup offer.",
            "Take the stable offer.",
        )
    }

    #[test]
    fn create_stores_round_and_valid_slot() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, soul_a, base_a, _) = base_args(&soul_id);
        let row = create_evaluation(
            &env.conn,
            sid,
            scen,
            "Which job offer would you take?",
            domain,
            soul_a,
            base_a,
            "Short profile.",
            "SOUL CONTEXT\n...",
            &["ent_1".to_string(), "ent_2".to_string()],
        )
        .unwrap();
        assert!(row.id.starts_with("evl_"));
        assert!(row.soul_variant == "a" || row.soul_variant == "b");
        assert_eq!(row.user_choice, None);
        assert_eq!(row.completed_at, None);
        assert_eq!(
            row.context_entity_ids,
            vec!["ent_1".to_string(), "ent_2".to_string()]
        );
        assert_eq!(row.context_pack, "SOUL CONTEXT\n...");
        assert!(!row.created_at.is_empty());

        let listed = list_evaluations(&env.conn, &soul_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, row.id);
        assert_eq!(listed[0].soul_answer, soul_a);
    }

    #[test]
    fn create_rejects_missing_soul() {
        let env = TestEnv::new();
        let err = create_evaluation(
            &env.conn,
            "soul_missing",
            "scen_1",
            "Question?",
            "career",
            "a",
            "b",
            "",
            "",
            &[],
        )
        .unwrap_err();
        assert!(err.contains("SOUL not found"), "unexpected error: {err}");
    }

    #[test]
    fn create_rejects_empty_or_oversized_answers() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, soul_a, base_a, _) = base_args(&soul_id);
        let err = create_evaluation(
            &env.conn,
            sid,
            scen,
            "Q?",
            domain,
            "   ",
            base_a,
            "",
            "",
            &[],
        )
        .unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected error: {err}");

        let huge = "x".repeat(MAX_VARIANT_ANSWER_CHARS + 1);
        let err = create_evaluation(
            &env.conn,
            sid,
            scen,
            "Q?",
            domain,
            &huge,
            base_a,
            "",
            "",
            &[],
        )
        .unwrap_err();
        assert!(
            err.contains("soul_answer exceeds"),
            "unexpected error: {err}"
        );

        let err = create_evaluation(
            &env.conn,
            sid,
            scen,
            "Q?",
            domain,
            soul_a,
            &huge,
            "",
            "",
            &[],
        )
        .unwrap_err();
        assert!(
            err.contains("baseline_answer exceeds"),
            "unexpected error: {err}"
        );

        let err = create_evaluation(
            &env.conn,
            sid,
            scen,
            "Q?",
            domain,
            soul_a,
            base_a,
            "",
            &"p".repeat(MAX_CONTEXT_PACK_CHARS + 1),
            &[],
        )
        .unwrap_err();
        assert!(
            err.contains("context_pack exceeds"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn slot_pick_is_random_and_deterministic_per_seed() {
        let mut seen: Vec<&str> = Vec::new();
        for seed in 1..=8u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let slot = pick_slot(&mut rng);
            assert!(slot == "a" || slot == "b");
            let mut rng2 = StdRng::seed_from_u64(seed);
            assert_eq!(pick_slot(&mut rng2), slot, "same seed must give same slot");
            seen.push(slot);
        }
        assert!(
            seen.contains(&"a") && seen.contains(&"b"),
            "slots across seeds must vary, got {seen:?}"
        );
    }

    #[test]
    fn submit_choice_records_once_and_reveals_from_slot() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, soul_a, base_a, _) = base_args(&soul_id);
        let row =
            create_evaluation_with_slot(&env.conn, sid, scen, "Q?", domain, soul_a, base_a, "a");
        let done = submit_choice(&env.conn, &row.id, "a").unwrap();
        assert_eq!(done.user_choice.as_deref(), Some("a"));
        assert!(done.completed_at.is_some());
        assert_eq!(done.soul_variant, "a");
        assert!(
            done.user_choice.as_deref() == Some(done.soul_variant.as_str()),
            "choice 'a' with slot 'a' means the SOUL variant won"
        );
    }

    #[test]
    fn submit_choice_is_final_and_validated() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, soul_a, base_a, _) = base_args(&soul_id);
        let row =
            create_evaluation_with_slot(&env.conn, sid, scen, "Q?", domain, soul_a, base_a, "b");

        let err = submit_choice(&env.conn, &row.id, "x").unwrap_err();
        assert!(err.contains("choice must be"), "unexpected error: {err}");

        submit_choice(&env.conn, &row.id, "neither").unwrap();
        let err = submit_choice(&env.conn, &row.id, "a").unwrap_err();
        assert!(err.contains("already completed"), "unexpected error: {err}");
        let reloaded = list_evaluations(&env.conn, &soul_id).unwrap().remove(0);
        assert_eq!(reloaded.user_choice.as_deref(), Some("neither"));
    }

    #[test]
    fn submit_choice_unknown_evaluation_fails() {
        let env = TestEnv::new();
        let err = submit_choice(&env.conn, "evl_nope", "a").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn list_filters_by_soul_and_orders_newest_first() {
        let env = TestEnv::new();
        let soul_a = seed_soul(&env);
        let soul_b = create_soul(&env.conn, "Другая", "device_e")
            .unwrap()
            .soul_id;
        let (sid, scen, domain, s_a, b_a, _) = base_args(&soul_a);
        let first = create_evaluation_with_slot(&env.conn, sid, scen, "Q1?", domain, s_a, b_a, "a");
        let second =
            create_evaluation_with_slot(&env.conn, sid, scen, "Q2?", domain, s_a, b_a, "b");
        create_evaluation_with_slot(&env.conn, &soul_b, scen, "Q3?", domain, s_a, b_a, "a");

        let list = list_evaluations(&env.conn, &soul_a).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].created_at >= list[1].created_at);
        let ids: Vec<&str> = list.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&first.id.as_str()) && ids.contains(&second.id.as_str()));
    }

    #[test]
    fn delete_removes_and_repeats_fail() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, s_a, b_a, _) = base_args(&soul_id);
        let row = create_evaluation_with_slot(&env.conn, sid, scen, "Q?", domain, s_a, b_a, "a");

        delete_evaluation(&env.conn, &row.id).unwrap();
        assert!(list_evaluations(&env.conn, &soul_id).unwrap().is_empty());

        let err = delete_evaluation(&env.conn, &row.id).unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn wipe_all_clears_evaluations_with_soul() {
        let env = TestEnv::new();
        let soul_id = seed_soul(&env);
        let (sid, scen, domain, s_a, b_a, _) = base_args(&soul_id);
        create_evaluation_with_slot(&env.conn, sid, scen, "Q?", domain, s_a, b_a, "a");
        assert_eq!(list_evaluations(&env.conn, &soul_id).unwrap().len(), 1);

        crate::db::wipe_all(&env.conn).unwrap();
        assert!(list_evaluations(&env.conn, &soul_id).unwrap().is_empty());
    }
}
