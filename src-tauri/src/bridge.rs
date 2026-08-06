//! Native Messaging host для Browser Companion (SESSION-09).
//!
//! Собственный узкий протокол `soul-bridge/1` поверх Chrome Native Messaging
//! (кадры u32-LE длина + JSON). Расширение SOUL подключается через
//! `chrome.runtime.connectNative("com.soul.browser_companion")`, отправляет
//! `soul.get_context` с задачей из поля ввода; host валидирует extension ID,
//! версию протокола, nonce (формат + replay), происхождение запроса и лимиты
//! размера, компилирует разрешённый контекст из локальной БД (read-only,
//! тот же компилятор, что и в MCP) и пишет disclosure-квитанцию без текста
//! задачи и секретов. Пак контекста никогда не попадает в stderr, логи и
//! расширенное хранилище.

use crate::context::{self, ContextQuery};
use crate::package::{self, DisclosureReceipt};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::path::Path;

pub const BRIDGE_HOST_NAME: &str = "com.soul.browser_companion";
pub const BRIDGE_PROTOCOL_VERSION: &str = "soul-bridge/1";
/// Chrome лимитирует одно native-сообщение 1 МБ — держим такой же предел.
pub const BRIDGE_MAX_FRAME_BYTES: u64 = 1024 * 1024;
/// Максимальная длина текста задачи из поля ввода веб-чата.
pub const BRIDGE_MAX_TASK_CHARS: usize = 8000;

/// Единственный зарегистрированный ID расширения. ID фиксирован ключом в
/// manifest.source.json расширения; при расхождении ключа и ID все запросы
/// отклоняются (fail-closed). В тестах можно переопределить через env
/// `SOUL_BRIDGE_ALLOWED_EXTENSION_IDS` (список через запятую).
pub const BRIDGE_EXTENSION_ID: &str = "epfbcmgajbpjbphepfbhcoibmoaflbld";

/// Разрешённые происхождения запроса: только поддерживаемые веб-чаты.
pub const SUPPORTED_ORIGINS: [&str; 3] = [
    "https://chatgpt.com",
    "https://gemini.google.com",
    "https://claude.ai",
];

/// Коды ошибок протокола (зеркалируют browser/src/protocol.ts).
pub const ERR_INVALID_PROTOCOL: &str = "invalid_protocol";
pub const ERR_INVALID_EXTENSION_ID: &str = "invalid_extension_id";
pub const ERR_INVALID_NONCE: &str = "invalid_nonce";
pub const ERR_INVALID_ORIGIN: &str = "invalid_origin";
pub const ERR_TASK_TOO_LONG: &str = "task_too_long";
pub const ERR_REQUEST_TOO_LARGE: &str = "request_too_large";
pub const ERR_REPLAY_DETECTED: &str = "replay_detected";
pub const ERR_INVALID_REQUEST: &str = "invalid_request";
pub const ERR_UNSUPPORTED_REQUEST: &str = "unsupported_request";
pub const ERR_RUNTIME_ERROR: &str = "runtime_error";

/// Открывает БД SOUL. Сначала — строго на чтение (host не должен писать
/// бизнес-данные); если read-only открытие невозможно (WAL требует доступа
/// к `-wal`/`-shm`), открывает read-write и запирает на чтение
/// `PRAGMA query_only=ON` — SQLite не примет ни одной записи в таблицы.
/// БД зашифрована SQLCipher (ключ — SHA-256 от ключа устройства).
pub fn open_app_db(app_dir: &Path) -> Result<Connection, String> {
    let db_path = app_dir.join("soul.db");
    let key_hex = hex::encode(crate::crypto::db_encryption_key(app_dir)?);
    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .or_else(|_| Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE))
        .map_err(|_| format!("SOUL database not found at {}.", db_path.to_string_lossy()))?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
        .map_err(|e| format!("Cannot unlock SOUL database: {e}"))?;
    conn.pragma_update(None, "query_only", true)
        .map_err(|e| format!("Cannot enforce read-only database access: {e}"))?;
    Ok(conn)
}

/// Список разрешённых extension ID: env-переопределение для тестов, иначе
/// единственный фиксированный ID.
pub fn allowed_extension_ids() -> Vec<String> {
    if let Ok(raw) = std::env::var("SOUL_BRIDGE_ALLOWED_EXTENSION_IDS") {
        let ids: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !ids.is_empty() {
            return ids;
        }
    }
    vec![BRIDGE_EXTENSION_ID.to_string()]
}

/// Валидный nonce: 16–64 символа [A-Za-z0-9_-].
pub fn is_valid_nonce(nonce: &str) -> bool {
    (16..=64).contains(&nonce.len())
        && nonce
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Каноническое происхождение (scheme://host). http и не-HTTP отклоняются.
pub fn is_supported_origin(origin: &str) -> bool {
    SUPPORTED_ORIGINS.contains(&origin)
}

fn error_response(nonce: Option<&str>, code: &str, message: impl Into<String>) -> Value {
    json!({
        "type": "soul.error",
        "protocol": BRIDGE_PROTOCOL_VERSION,
        "nonce": nonce,
        "ok": false,
        "code": code,
        "message": message.into(),
    })
}

/// Состояние одного соединения: защита от повторного nonce (replay).
#[derive(Default)]
pub struct BridgeSession {
    seen_nonces: HashSet<String>,
    nonce_order: VecDeque<String>,
}

const MAX_SESSION_NONCES: usize = 4_096;

impl BridgeSession {
    pub fn new() -> BridgeSession {
        BridgeSession::default()
    }

    fn register_nonce(&mut self, nonce: &str) -> bool {
        if self.seen_nonces.contains(nonce) {
            return false;
        }
        if self.nonce_order.len() >= MAX_SESSION_NONCES {
            if let Some(oldest) = self.nonce_order.pop_front() {
                self.seen_nonces.remove(&oldest);
            }
        }
        let nonce = nonce.to_string();
        self.seen_nonces.insert(nonce.clone());
        self.nonce_order.push_back(nonce);
        true
    }
}

/// Читает один native-кадр: 4 байта little-endian длина + payload.
/// EOF перед длиной → None (соединение закрыто). Кадр больше лимита — ошибка.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, String> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("stdin read failed: {e}")),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 {
        return Err("Empty native frame.".to_string());
    }
    if len as u64 > BRIDGE_MAX_FRAME_BYTES {
        return Err(format!(
            "Native frame too large: {len} bytes (limit {BRIDGE_MAX_FRAME_BYTES})."
        ));
    }
    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| format!("stdin read failed: {e}"))?;
    Ok(Some(payload))
}

/// Пишет один native-кадр. Ответы тоже ограничены 1 МБ.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), String> {
    if payload.len() as u64 > BRIDGE_MAX_FRAME_BYTES {
        return Err(format!(
            "Native response too large: {} bytes.",
            payload.len()
        ));
    }
    let len = payload.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .map_err(|e| format!("stdout write failed: {e}"))?;
    writer
        .write_all(payload)
        .map_err(|e| format!("stdout write failed: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("stdout flush failed: {e}"))
}

/// Непрерывный цикл host-процесса: stdin → кадры, stdout → кадры.
pub fn serve_native_messaging(app_dir: &Path) -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = std::io::BufWriter::new(stdout.lock());
    let mut session = BridgeSession::new();
    serve_frames(&mut reader, &mut writer, app_dir, &mut session)
}

/// Обработка потока кадров до EOF. На кадре, нарушающем границы протокола
/// (oversized, пустой), пишет ошибку и ЗАКРЫВАЕТ соединение: после такого
/// кадра поток stdin рассинхронизирован, продолжение читало бы мусор.
pub fn serve_frames<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    app_dir: &Path,
    session: &mut BridgeSession,
) -> Result<(), String> {
    loop {
        let frame = match read_frame(reader) {
            Ok(Some(frame)) => frame,
            Ok(None) => return Ok(()),
            Err(e) => {
                // Размер кадра не прошёл проверку ещё до парсинга: nonce
                // неизвестен, отвечаем общим отказом без содержимого.
                let response = error_response(None, ERR_REQUEST_TOO_LARGE, e);
                let payload = serde_json::to_vec(&response)
                    .map_err(|e| format!("response serialization failed: {e}"))?;
                write_frame(writer, &payload)?;
                return Err("Native frame rejected: closing connection.".to_string());
            }
        };
        if let Some(response) = handle_frame(&frame, app_dir, session) {
            let payload = serde_json::to_vec(&response)
                .map_err(|e| format!("response serialization failed: {e}"))?;
            write_frame(writer, &payload)?;
        }
    }
}

/// Обработка одного кадра. None — ответ не требуется (не бывает в этом
/// протоколе: каждый запрос имеет ответ).
pub fn handle_frame(frame: &[u8], app_dir: &Path, session: &mut BridgeSession) -> Option<Value> {
    let parsed: Value = match serde_json::from_slice(frame) {
        Ok(v) => v,
        Err(_) => {
            return Some(error_response(
                None,
                ERR_INVALID_REQUEST,
                "Invalid JSON frame.",
            ));
        }
    };
    let Some(msg) = parsed.as_object() else {
        return Some(error_response(None, ERR_INVALID_REQUEST, "Invalid Request"));
    };

    // Протокол: первым делом — версия, затем общие поля.
    if msg.get("protocol").and_then(|v| v.as_str()) != Some(BRIDGE_PROTOCOL_VERSION) {
        let nonce = msg.get("nonce").and_then(|v| v.as_str());
        return Some(error_response(
            nonce,
            ERR_INVALID_PROTOCOL,
            "Unsupported protocol version.",
        ));
    }
    let Some(extension_id) = msg.get("extensionId").and_then(|v| v.as_str()) else {
        let nonce = msg.get("nonce").and_then(|v| v.as_str());
        return Some(error_response(
            nonce,
            ERR_INVALID_EXTENSION_ID,
            "Missing extension ID.",
        ));
    };
    if !allowed_extension_ids().iter().any(|id| id == extension_id) {
        let nonce = msg.get("nonce").and_then(|v| v.as_str());
        return Some(error_response(
            nonce,
            ERR_INVALID_EXTENSION_ID,
            "Unknown extension ID.",
        ));
    }
    let Some(nonce) = msg.get("nonce").and_then(|v| v.as_str()) else {
        return Some(error_response(None, ERR_INVALID_NONCE, "Missing nonce."));
    };
    if !is_valid_nonce(nonce) {
        return Some(error_response(
            Some(nonce),
            ERR_INVALID_NONCE,
            "Malformed nonce.",
        ));
    }
    if !session.register_nonce(nonce) {
        return Some(error_response(
            Some(nonce),
            ERR_REPLAY_DETECTED,
            "Nonce already used.",
        ));
    }

    let request_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or_default();
    match request_type {
        "soul.ping" => Some(json!({
            "type": "soul.pong",
            "protocol": BRIDGE_PROTOCOL_VERSION,
            "nonce": nonce,
            "ok": true,
        })),
        "soul.get_context" => Some(handle_get_context(msg, nonce, app_dir)),
        other => Some(error_response(
            Some(nonce),
            ERR_UNSUPPORTED_REQUEST,
            format!("Unsupported request type: {other}"),
        )),
    }
}

fn handle_get_context(msg: &serde_json::Map<String, Value>, nonce: &str, app_dir: &Path) -> Value {
    let Some(origin) = msg.get("origin").and_then(|v| v.as_str()) else {
        return error_response(Some(nonce), ERR_INVALID_ORIGIN, "Missing origin.");
    };
    if !is_supported_origin(origin) {
        return error_response(Some(nonce), ERR_INVALID_ORIGIN, "Unsupported origin.");
    }

    let task = msg.get("task").and_then(|v| v.as_str()).unwrap_or_default();
    if task.chars().count() > BRIDGE_MAX_TASK_CHARS {
        return error_response(
            Some(nonce),
            ERR_TASK_TOO_LONG,
            format!("Task text exceeds {} characters.", BRIDGE_MAX_TASK_CHARS),
        );
    }
    // maxTokens: необязательный, целое 1..=3000 (как в protocol.ts); вне
    // диапазона или дробное — ошибка клиента.
    let max_tokens = match msg.get("maxTokens") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => match n.as_u64() {
            Some(v) if (1..=context::CONTEXT_HARD_MAX_TOKENS).contains(&v) => Some(v as f64),
            _ => {
                return error_response(
                    Some(nonce),
                    ERR_INVALID_REQUEST,
                    format!(
                        "maxTokens must be an integer between 1 and {}.",
                        context::CONTEXT_HARD_MAX_TOKENS
                    ),
                )
            }
        },
        Some(_) => {
            return error_response(
                Some(nonce),
                ERR_INVALID_REQUEST,
                "maxTokens must be a number.",
            )
        }
    };

    let query = ContextQuery {
        text: task.trim().to_string(),
        max_tokens,
        ..ContextQuery::default()
    };

    match compile_and_respond(app_dir, origin, &query) {
        Ok(value) => {
            let mut response = value;
            response["nonce"] = json!(nonce);
            response["protocol"] = json!(BRIDGE_PROTOCOL_VERSION);
            response
        }
        Err(e) => error_response(Some(nonce), ERR_RUNTIME_ERROR, e),
    }
}

/// Читает БД (read-only), компилирует пак и пишет disclosure-квитанцию.
/// Ответ не содержит nonce — его добавляет вызывающий.
pub fn compile_and_respond(
    app_dir: &Path,
    origin: &str,
    query: &ContextQuery,
) -> Result<Value, String> {
    context::validate_query(query)?;
    let conn = open_app_db(app_dir)?;
    let pack = context::compile_context_cached(&conn, query)?;

    let receipt = DisclosureReceipt {
        kind: "disclosure".to_string(),
        disclosed_at: chrono::Utc::now().to_rfc3339(),
        client: format!("browser-companion:{origin}"),
        entity_count: pack.items.len() as i64,
        token_estimate: pack.token_estimate as i64,
        policy_version: pack.policy_version.clone(),
        state_version: pack.state_version.clone(),
        max_tokens: pack.max_tokens as i64,
        cost_estimate_usd: context::cost_estimate_usd(pack.token_estimate),
    };
    package::write_disclosure_receipt(app_dir, &receipt)
        .map_err(|e| format!("Cannot write disclosure receipt: {e}"))?;

    Ok(json!({
        "type": "soul.context",
        "ok": true,
        "pack": pack.serialized,
        "entityCount": pack.items.len(),
        "tokenEstimate": pack.token_estimate,
        "costEstimateUsd": context::cost_estimate_usd(pack.token_estimate),
        "policyVersion": pack.policy_version,
        "stateVersion": pack.state_version,
        "maxTokens": pack.max_tokens,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{add_entity, create_soul, init_db};
    use std::io::Cursor;

    struct TestEnv {
        dir: std::path::PathBuf,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir =
                std::env::temp_dir().join(format!("soul-bridge-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            let soul = create_soul(&conn, "Тест", "device_b").unwrap();
            add_entity(
                &conn,
                &soul.soul_id,
                "preference",
                "active",
                r#"{"claim":"Prefers concise answers","evidence":"stated","questionId":"pref_1","value":"concise","confidence":0.9,"sensitivity":"internal","scope":{"domains":["preferences"],"projects":[],"people":[],"channels":[]}}"#,
                "device_b",
            )
            .unwrap();
            add_entity(
                &conn,
                &soul.soul_id,
                "boundary",
                "active",
                r#"{"claim":"Never share medical data","questionId":"bound_1","value":"never","confidence":0.8,"sensitivity":"sensitive","scope":{"domains":["boundaries"],"projects":[],"people":[],"channels":[]}}"#,
                "device_b",
            )
            .unwrap();
            TestEnv { dir }
        }

        fn send(&self, json: &str, session: &mut BridgeSession) -> Value {
            handle_frame(json.as_bytes(), &self.dir, session).expect("response")
        }

        /// Строит валидный запрос с JSON-переопределениями любых полей.
        fn valid_request(&self, overrides: &[(&str, Value)]) -> String {
            let mut body = json!({
                "type": "soul.get_context",
                "protocol": BRIDGE_PROTOCOL_VERSION,
                "extensionId": BRIDGE_EXTENSION_ID,
                "nonce": "test_nonce_000000000000001",
                "origin": "https://chatgpt.com",
                "task": "concise answers",
                "maxTokens": 900,
            });
            for (key, value) in overrides {
                body[key] = value.clone();
            }
            body.to_string()
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn receipt_texts(env: &TestEnv) -> Vec<String> {
        let dir = env.dir.join("receipts");
        if !dir.exists() {
            return Vec::new();
        }
        std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect()
    }

    #[test]
    fn read_write_frame_roundtrip() {
        let mut reader = Cursor::new(vec![3, 0, 0, 0, b'a', b'b', b'c']);
        assert_eq!(read_frame(&mut reader).unwrap(), Some(b"abc".to_vec()));
        assert!(read_frame(&mut reader).unwrap().is_none());

        let mut out: Vec<u8> = Vec::new();
        write_frame(&mut out, b"hello").unwrap();
        assert_eq!(out, vec![5, 0, 0, 0, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn read_frame_rejects_oversized_and_empty() {
        let mut reader = Cursor::new(vec![0xff, 0xff, 0xff, 0x3f]);
        let err = read_frame(&mut reader).unwrap_err();
        assert!(err.contains("too large"));

        let mut reader = Cursor::new(vec![0, 0, 0, 0]);
        assert!(read_frame(&mut reader).unwrap_err().contains("Empty"));
    }

    #[test]
    fn valid_get_context_returns_pack_and_writes_receipt() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let res = env.send(&env.valid_request(&[("task", json!(""))]), &mut session);
        assert_eq!(res["type"], "soul.context");
        assert_eq!(res["ok"], true);
        assert_eq!(res["nonce"], "test_nonce_000000000000001");
        assert_eq!(res["protocol"], BRIDGE_PROTOCOL_VERSION);
        assert_eq!(res["entityCount"], 1);
        assert!(res["pack"].as_str().unwrap().starts_with("SOUL CONTEXT"));

        let receipts = receipt_texts(&env);
        assert_eq!(receipts.len(), 1);
        let text = &receipts[0];
        assert!(text.contains("\"client\": \"browser-companion:https://chatgpt.com\""));
        assert!(!text.contains("Prefers concise answers"));
        assert!(!text.contains("Never share medical data"));
        assert!(!text.contains("ent_"));
        assert!(!text.contains("concise"));
    }

    #[test]
    fn rejects_unknown_extension_id() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let res = env.send(
            &env.valid_request(&[("extensionId", json!("a".repeat(32)))]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_EXTENSION_ID);
        assert_eq!(res["ok"], false);
        assert!(receipt_texts(&env).is_empty());
    }

    #[test]
    fn rejects_wrong_protocol_version() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let json = env
            .valid_request(&[])
            .replacen(BRIDGE_PROTOCOL_VERSION, "soul-bridge/0", 1);
        let res = env.send(&json, &mut session);
        assert_eq!(res["code"], ERR_INVALID_PROTOCOL);
        assert!(receipt_texts(&env).is_empty());
    }

    #[test]
    fn rejects_malformed_and_replayed_nonce() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();

        let res = env.send(
            &env.valid_request(&[("nonce", json!("short"))]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_NONCE);

        let res = env.send(
            &env.valid_request(&[("nonce", json!("bad nonce with spaces!"))]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_NONCE);

        let first = env.valid_request(&[("nonce", json!("nonce_replay_0000000000000000"))]);
        assert_eq!(env.send(&first, &mut session)["ok"], true);
        let replay = env.valid_request(&[("nonce", json!("nonce_replay_0000000000000000"))]);
        let res = env.send(&replay, &mut session);
        assert_eq!(res["code"], ERR_REPLAY_DETECTED);
    }

    #[test]
    fn rejects_unsupported_origin() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();

        let res = env.send(
            &env.valid_request(&[
                ("origin", json!("https://evil.example")),
                ("nonce", json!("origin_evil_00000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_ORIGIN);

        let res = env.send(
            &env.valid_request(&[
                ("origin", json!("http://chatgpt.com")),
                ("nonce", json!("origin_http_00000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_ORIGIN);

        let res = env.send(
            &env.valid_request(&[
                ("origin", json!("chrome-extension://abc/")),
                ("nonce", json!("origin_ext_000000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_ORIGIN);
        assert!(receipt_texts(&env).is_empty());
    }

    #[test]
    fn rejects_oversized_task() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let huge = "a".repeat(BRIDGE_MAX_TASK_CHARS + 1);
        let json = env.valid_request(&[]).replacen(
            "\"task\":\"concise answers\"",
            &format!("\"task\":\"{huge}\""),
            1,
        );
        let res = env.send(&json, &mut session);
        assert_eq!(res["code"], ERR_TASK_TOO_LONG);
        assert!(receipt_texts(&env).is_empty());
    }

    #[test]
    fn task_filters_irrelevant_entities() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let json = env.valid_request(&[]).replacen(
            "\"task\":\"concise answers\"",
            "\"task\":\"xylophone\"",
            1,
        );
        let res = env.send(&json, &mut session);
        assert_eq!(res["entityCount"], 0);
        assert!(res["pack"].as_str().unwrap().contains("entities: 0"));
        // Квитанция всё равно пишется: раскрытие нулевого объёма тоже факт.
        assert_eq!(receipt_texts(&env).len(), 1);
    }

    #[test]
    fn max_tokens_is_validated_and_clamped() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();

        let res = env.send(
            &env.valid_request(&[
                ("maxTokens", json!(99999)),
                ("nonce", json!("max_high_000000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_REQUEST);

        let res = env.send(
            &env.valid_request(&[
                ("maxTokens", json!(0)),
                ("nonce", json!("max_zero_00000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_REQUEST);

        let res = env.send(
            &env.valid_request(&[
                ("maxTokens", json!(1)),
                ("nonce", json!("max_one_000000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["ok"], true);
        assert_eq!(res["maxTokens"], 1);
    }

    #[test]
    fn ping_gets_pong() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let json = format!(
            r#"{{"type":"soul.ping","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"ping_nonce_0000000000000000"}}"#
        );
        let res = env.send(&json, &mut session);
        assert_eq!(res["type"], "soul.pong");
        assert_eq!(res["ok"], true);
        assert_eq!(res["nonce"], "ping_nonce_0000000000000000");
    }

    #[test]
    fn unknown_request_type_is_rejected() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let json = format!(
            r#"{{"type":"soul.wipe_all","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"wipe_nonce_0000000000000000"}}"#
        );
        let res = env.send(&json, &mut session);
        assert_eq!(res["code"], ERR_UNSUPPORTED_REQUEST);
    }

    #[test]
    fn invalid_json_frame_is_rejected() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let res = env.send("not json", &mut session);
        assert_eq!(res["code"], ERR_INVALID_REQUEST);
        assert_eq!(res["nonce"], Value::Null);
    }

    #[test]
    fn nonce_validation_rules() {
        assert!(is_valid_nonce("a".repeat(16).as_str()));
        assert!(is_valid_nonce("a".repeat(64).as_str()));
        assert!(is_valid_nonce("abc_DEF-0123456789abcdef"));
        assert!(!is_valid_nonce("a".repeat(15).as_str()));
        assert!(!is_valid_nonce("a".repeat(65).as_str()));
        assert!(!is_valid_nonce("nonce with space"));
        assert!(!is_valid_nonce("не-ascii-нонс"));
    }

    #[test]
    fn replay_cache_is_bounded() {
        let mut session = BridgeSession::new();
        for index in 0..(MAX_SESSION_NONCES + 20) {
            assert!(session.register_nonce(&format!("nonce_{index:016}")));
        }
        assert_eq!(session.seen_nonces.len(), MAX_SESSION_NONCES);
        assert_eq!(session.nonce_order.len(), MAX_SESSION_NONCES);
        assert!(session.register_nonce("nonce_0000000000000000"));
        assert!(!session.register_nonce("nonce_0000000000000021"));
    }

    #[test]
    fn serve_loop_roundtrips_over_frames() {
        let env = TestEnv::new();
        let mut input: Vec<u8> = Vec::new();
        let ping = format!(
            r#"{{"type":"soul.ping","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"serve_ping_0000000000000000"}}"#
        );
        let ctx = env.valid_request(&[("nonce", json!("serve_ctx_00000000000000000"))]);
        for frame in [ping.into_bytes(), ctx.into_bytes()] {
            input.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            input.extend_from_slice(&frame);
        }
        let mut reader = Cursor::new(input);
        let mut out: Vec<u8> = Vec::new();
        serve_frames(&mut reader, &mut out, &env.dir, &mut BridgeSession::new()).unwrap();
        let mut out_reader = Cursor::new(out);
        let first = read_frame(&mut out_reader).unwrap().unwrap();
        let first_json: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(first_json["type"], "soul.pong");
        let second = read_frame(&mut out_reader).unwrap().unwrap();
        let second_json: Value = serde_json::from_slice(&second).unwrap();
        assert_eq!(second_json["type"], "soul.context");
        assert!(read_frame(&mut out_reader).unwrap().is_none());
    }

    #[test]
    fn serve_loop_closes_on_oversized_frame() {
        let env = TestEnv::new();
        // 0x3fffffff байт — больше лимита 1 МБ; после него поток рассинхронен.
        let input: Vec<u8> = vec![0xff, 0xff, 0xff, 0x3f];
        let mut reader = Cursor::new(input);
        let mut out: Vec<u8> = Vec::new();
        let result = serve_frames(&mut reader, &mut out, &env.dir, &mut BridgeSession::new());
        assert!(
            result.is_err(),
            "соединение должно закрыться на oversized-кадре"
        );
        let mut out_reader = Cursor::new(out);
        let first = read_frame(&mut out_reader).unwrap().unwrap();
        let first_json: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(first_json["type"], "soul.error");
        assert_eq!(first_json["code"], ERR_REQUEST_TOO_LARGE);
        assert!(read_frame(&mut out_reader).unwrap().is_none());
    }

    #[test]
    fn fractional_max_tokens_is_rejected() {
        let env = TestEnv::new();
        let mut session = BridgeSession::new();
        let res = env.send(
            &env.valid_request(&[
                ("maxTokens", json!(2.5)),
                ("nonce", json!("max_frac_000000000000000000")),
            ]),
            &mut session,
        );
        assert_eq!(res["code"], ERR_INVALID_REQUEST);
        assert!(receipt_texts(&env).is_empty());
    }

    /// E2E: реальный бинарь soul-bridge.exe по кадрам native messaging.
    #[test]
    #[ignore = "run through pnpm release:check after building release sidecars"]
    fn real_binary_serves_native_messaging() {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let env = TestEnv::new();
        let exe = std::env::current_exe().unwrap();
        let mut bin = exe.parent().unwrap().parent().unwrap().to_path_buf();
        bin.push(if cfg!(windows) {
            "soul-bridge.exe"
        } else {
            "soul-bridge"
        });
        if !bin.exists() {
            eprintln!(
                "soul-bridge binary not built; skipping E2E test ({})",
                bin.display()
            );
            return;
        }

        let mut child = Command::new(&bin)
            .env("SOUL_APP_DIR", &env.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn soul-bridge");

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

        let mut request = |stdin: &mut std::process::ChildStdin, frame: &[u8]| {
            stdin
                .write_all(&(frame.len() as u32).to_le_bytes())
                .unwrap();
            stdin.write_all(frame).unwrap();
            stdin.flush().unwrap();
            let mut len_buf = [0u8; 4];
            stdout.read_exact(&mut len_buf).unwrap();
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            stdout.read_exact(&mut payload).unwrap();
            serde_json::from_slice::<Value>(&payload).unwrap()
        };

        let ping = format!(
            r#"{{"type":"soul.ping","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"e2e_ping_00000000000000000"}}"#
        );
        let res = request(&mut stdin, ping.as_bytes());
        assert_eq!(res["type"], "soul.pong");

        let ctx = format!(
            r#"{{"type":"soul.get_context","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"e2e_ctx_000000000000000000","origin":"https://gemini.google.com","task":"concise","maxTokens":900}}"#
        );
        let res = request(&mut stdin, ctx.as_bytes());
        assert_eq!(res["type"], "soul.context");
        assert_eq!(
            res["entityCount"], 1,
            "only the preference matches 'concise'"
        );
        assert!(res["pack"]
            .as_str()
            .unwrap()
            .contains("Prefers concise answers"));
        assert!(!res["pack"]
            .as_str()
            .unwrap()
            .contains("Never share medical data"));

        let receipts = receipt_texts(&env);
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].contains("browser-companion:https://gemini.google.com"));

        drop(stdin);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "soul-bridge exited with {status}");
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "soul-bridge did not exit on stdin EOF"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn empty_database_yields_empty_pack() {
        let dir = std::env::temp_dir().join(format!("soul-bridge-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init_db(&dir).unwrap();
        let json = format!(
            r#"{{"type":"soul.get_context","protocol":"{BRIDGE_PROTOCOL_VERSION}","extensionId":"{BRIDGE_EXTENSION_ID}","nonce":"empty_ctx_0000000000000000","origin":"https://claude.ai","task":"","maxTokens":900}}"#
        );
        let res = handle_frame(json.as_bytes(), &dir, &mut BridgeSession::new()).unwrap();
        assert_eq!(res["type"], "soul.context");
        assert_eq!(res["entityCount"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_app_db_fails_when_database_missing() {
        let dir = std::env::temp_dir().join(format!("soul-bridge-nodb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(open_app_db(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
