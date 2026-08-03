//! Локальный MCP-сервер (stdio transport, JSON-RPC 2.0, newline-delimited).
//!
//! Узкий API: ровно один инструмент `soul.get_context` + один промпт
//! `soul.task_start` с инструкциями. Сервер никогда не пишет в БД (read-only),
//! не вызывает модели и не ведёт собственный чат. Каждое раскрытие контекста
//! фиксируется локальной disclosure-квитанцией без текста задачи и секретов.
//!
//! Собственный протокол вместо внешнего MCP-крейта: зависимостей меньше,
//! поведение полностью контролируется и покрыто тестами.

use crate::context::{self, ContextQuery};
use crate::db;
use crate::package::{self, DisclosureReceipt};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::path::Path;

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "soul-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TOOL_GET_CONTEXT: &str = "soul.get_context";
pub const PROMPT_TASK_START: &str = "soul.task_start";

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;

/// Текст инструкций для ассистента (промпт `soul.task_start`). Требует
/// запрашивать контекст в начале подходящей задачи, но явно не выдаёт
/// добровольный вызов за гарантированный enforcement.
const TASK_START_INSTRUCTIONS: &str = r#"SOUL is a local, personal knowledge base about this user. A local MCP server exposes the tool `soul.get_context`.

Instructions:

1. At the START of a task that involves this user's preferences, boundaries, goals, decisions or facts (planning, writing, reviewing, deciding), call `soul.get_context` once and use the returned context as background for the task.
2. The call is voluntary: for tasks with no such relevance (syntax, math, general knowledge), you do not need to call the tool.
3. The pack may contain conflicts and superseded items. Conflicts mean the user changed their mind on that calibration question — prefer the most recent answer. Superseded items must not be treated as current facts.
4. Never invent context that is not in the returned pack. If the pack is empty, act without it.
5. The pack is a local read: it never leaves this machine."#;

fn get_context_tool_spec() -> Value {
    json!({
        "name": TOOL_GET_CONTEXT,
        "description": "Get the minimal permitted local context about the user for the current task. The pack is compiled deterministically from the local SOUL database: only active entities, sensitivity below restricted by default, filtered by scope and time, deduplicated, with explicit conflicts and a token budget (default 900, hard maximum 3000). Returns the serialized pack as text plus a JSON metadata block with counts, versions and superseded ids. SOUL never reads or modifies the task; calling this tool is voluntary. Every call is recorded in a local disclosure receipt that contains no task text and no secrets.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Task text used for relevance filtering. Entities with no term overlap are excluded from the pack."
                },
                "domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed domains (e.g. preferences, boundaries, goals, decisions, writing, personal). Empty = no restriction."
                },
                "projects": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed project scope values. Empty = no restriction."
                },
                "people": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed people scope values. Empty = no restriction."
                },
                "channels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed channel scope values. Empty = no restriction."
                },
                "sensitivity": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["public", "internal", "private", "sensitive", "restricted"]
                    },
                    "description": "Allowed sensitivity levels. Empty = all except restricted."
                },
                "statuses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed entity statuses. Empty = only active."
                },
                "since": {
                    "type": "string",
                    "description": "ISO/RFC3339 lower bound for entity created_at (inclusive)."
                },
                "until": {
                    "type": "string",
                    "description": "ISO/RFC3339 upper bound for entity created_at (inclusive)."
                },
                "maxTokens": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3000,
                    "default": 900,
                    "description": "Token budget for the packed context."
                }
            }
        }
    })
}

/// Открывает БД SOUL только на чтение (никаких записей из MCP-процесса).
/// Фолбэк на read-write-открытие без CREATE нужен для WAL-баз, где
/// read-only-коннект невозможен из-за отсутствующих -shm файлов; фолбэк
/// немедленно запирается на чтение PRAGMA query_only=ON. БД зашифрована
/// SQLCipher (ключ — SHA-256 от ключа устройства).
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

/// Резолв app_dir для MCP-процесса: SOUL_APP_DIR (тесты/отладка) →
/// %APPDATA%/ai.soul.runtime (Windows), иначе ~/.local/share/ai.soul.runtime.
pub fn resolve_app_dir() -> Result<std::path::PathBuf, String> {
    if let Ok(dir) = std::env::var("SOUL_APP_DIR") {
        if !dir.trim().is_empty() {
            return Ok(std::path::PathBuf::from(dir));
        }
    }
    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Ok(std::path::PathBuf::from(appdata).join("ai.soul.runtime"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(std::path::PathBuf::from(home).join(".local/share/ai.soul.runtime"));
    }
    Err("Cannot resolve SOUL app data directory. Set SOUL_APP_DIR.".to_string())
}

/// Клиент, от имени которого сделан вызов. По умолчанию "unknown" — сервер не
/// угадывает и не подглядывает; клиенты могут передавать имя через env.
fn client_name() -> String {
    std::env::var("SOUL_MCP_CLIENT").unwrap_or_else(|_| "unknown".to_string())
}

/// Непрерывный цикл stdio: строки JSON-RPC 2.0 в stdin, ответы в stdout.
pub fn serve_stdio(app_dir: &Path) -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = std::io::BufWriter::new(stdout.lock());
    serve_io(&mut reader, &mut writer, app_dir)
}

/// Максимальная длина одной строки JSON-RPC на stdin (дефолтный лимит MCP
/// SDK — 4 МБ). Строки длиннее отклоняются с parse-ошибкой, цикл не рвётся.
pub const MCP_MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Тестируемая обвязка: читает строки из reader, пишет ответы в writer.
pub fn serve_io<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    app_dir: &Path,
) -> Result<(), String> {
    let mut line: Vec<u8> = Vec::new();
    loop {
        line.clear();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| format!("stdin read failed: {e}"))?;
        if n == 0 {
            return Ok(());
        }
        if line.len() > MCP_MAX_LINE_BYTES {
            if line.last() != Some(&b'\n') {
                // Досчитываем остаток строки, чтобы не разъехаться по кадрам.
                let mut drain: Vec<u8> = Vec::new();
                let _ = reader
                    .take(MCP_MAX_LINE_BYTES as u64 + 1)
                    .read_to_end(&mut drain);
            }
            write_line(
                writer,
                &rpc_error(Value::Null, JSONRPC_PARSE_ERROR, "Request line too large"),
            )?;
            continue;
        }
        let line_str = String::from_utf8_lossy(&line);
        if let Some(response) = handle_line(&line_str, app_dir) {
            write_line(writer, &response)?;
        }
    }
}

fn write_line<W: Write>(writer: &mut W, response: &Value) -> Result<(), String> {
    let json = serde_json::to_string(response)
        .map_err(|e| format!("response serialization failed: {e}"))?;
    writeln!(writer, "{json}").map_err(|e| format!("stdout write failed: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("stdout flush failed: {e}"))
}

/// Обработка одной строки входящего JSON-RPC сообщения. Notification (без id)
/// возвращает None — ответ не пишется.
pub fn handle_line(line: &str, app_dir: &Path) -> Option<Value> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": JSONRPC_PARSE_ERROR,
                    "message": "Parse error: invalid JSON"
                }
            }));
        }
    };
    let Some(msg) = parsed.as_object() else {
        return Some(rpc_error(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request",
        ));
    };
    if msg.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Some(rpc_error(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request",
        ));
    }
    let id = msg.get("id").cloned();
    let Some(id) = id else {
        return None; // notification — ответа не требуется
    };
    let Some(method) = msg.get("method").and_then(|m| m.as_str()) else {
        return Some(rpc_error(id, JSONRPC_INVALID_REQUEST, "Invalid Request"));
    };

    let response = match method {
        "initialize" => Ok(json!({ "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": { "tools": {}, "prompts": {} },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION } })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": [get_context_tool_spec()] })),
        "tools/call" => Ok(handle_tool_call(msg, app_dir)),
        "prompts/list" => Ok(json!({ "prompts": [{
            "name": PROMPT_TASK_START,
            "description": "Instructions for the assistant: request local SOUL context at the start of a suitable task.",
            "arguments": []
        }] })),
        "prompts/get" => handle_prompt_get(msg),
        _ => Err((
            JSONRPC_METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    };
    Some(match response {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => rpc_error(id, code, message),
    })
}

fn rpc_error<V: Into<Value>>(id: Value, code: i64, message: V) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn handle_prompt_get(msg: &serde_json::Map<String, Value>) -> Result<Value, (i64, String)> {
    let name = msg
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str());
    if name != Some(PROMPT_TASK_START) {
        return Err((JSONRPC_INVALID_PARAMS, format!("Unknown prompt: {name:?}")));
    }
    Ok(json!({
        "description": "Instructions for the assistant: request local SOUL context at the start of a suitable task.",
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": TASK_START_INSTRUCTIONS }
        }]
    }))
}

fn handle_tool_call(msg: &serde_json::Map<String, Value>, app_dir: &Path) -> Value {
    let Some(params) = msg.get("params").and_then(|p| p.as_object()).cloned() else {
        return tool_error("Missing params for tools/call.");
    };
    let Some(name) = params.get("name").and_then(|n| n.as_str()) else {
        return tool_error("Missing tool name.");
    };
    if name != TOOL_GET_CONTEXT {
        return tool_error(format!("Unknown tool: {name}"));
    }

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let query: ContextQuery = match serde_json::from_value(arguments) {
        Ok(q) => q,
        Err(e) => return tool_error(format!("Invalid arguments for {TOOL_GET_CONTEXT}: {e}")),
    };

    match get_context(app_dir, &query) {
        Ok(result) => result,
        Err(e) => tool_error(e),
    }
}

/// Ядро `soul.get_context`: read-only чтение БД → детерминированная компиляция
/// → disclosure receipt → ответ клиенту.
pub fn get_context(app_dir: &Path, query: &ContextQuery) -> Result<Value, String> {
    context::validate_query(query)?;
    let conn = open_app_db(app_dir)?;
    let entities: Vec<context::ContextEntity> = {
        let souls = db::list_souls(&conn).map_err(|e| format!("Cannot read SOUL database: {e}"))?;
        match souls.first() {
            Some(soul) => db::list_entities(&conn, &soul.soul_id)
                .map_err(|e| format!("Cannot read SOUL database: {e}"))?
                .into_iter()
                .map(|r| context::ContextEntity {
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
        }
    };

    let pack = context::compile_context(&entities, query);

    let receipt = DisclosureReceipt {
        kind: "disclosure".to_string(),
        disclosed_at: chrono::Utc::now().to_rfc3339(),
        client: client_name(),
        entity_count: pack.items.len() as i64,
        token_estimate: pack.token_estimate as i64,
        policy_version: pack.policy_version.clone(),
        state_version: pack.state_version.clone(),
        max_tokens: pack.max_tokens as i64,
    };
    package::write_disclosure_receipt(app_dir, &receipt)
        .map_err(|e| format!("Cannot write disclosure receipt: {e}"))?;

    let metadata = json!({
        "policyVersion": pack.policy_version,
        "stateVersion": pack.state_version,
        "entityCount": pack.items.len(),
        "maxTokens": pack.max_tokens,
        "tokenEstimate": pack.token_estimate,
        "conflicts": pack.conflicts,
        "supersededIds": pack.superseded_ids
    });
    Ok(json!({
        "content": [
            { "type": "text", "text": pack.serialized },
            { "type": "text", "text": metadata.to_string() }
        ]
    }))
}

fn tool_error(message: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": message.into() }],
        "isError": true
    })
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
            let dir = std::env::temp_dir().join(format!("soul-mcp-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = init_db(&dir).unwrap();
            let soul = create_soul(&conn, "Тест", "device_m").unwrap();
            add_entity(
                &conn,
                &soul.soul_id,
                "preference",
                "active",
                r#"{"claim":"Prefers concise answers","evidence":"stated","questionId":"pref_1","value":"concise","confidence":0.9,"sensitivity":"internal","scope":{"domains":["preferences"],"projects":[],"people":[],"channels":[]}}"#,
                "device_m",
            )
            .unwrap();
            add_entity(
                &conn,
                &soul.soul_id,
                "boundary",
                "active",
                r#"{"claim":"Never share medical data","questionId":"bound_1","value":"never","confidence":0.8,"sensitivity":"sensitive","scope":{"domains":["boundaries"],"projects":[],"people":[],"channels":[]}}"#,
                "device_m",
            )
            .unwrap();
            TestEnv { dir }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn send(env: &TestEnv, line: &str) -> Option<Value> {
        handle_line(line, &env.dir)
    }

    #[test]
    fn initialize_returns_capabilities_and_server_info() {
        let env = TestEnv::new();
        let res = send(
            &env,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        )
        .expect("response");
        assert_eq!(res["id"], 1);
        assert_eq!(res["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(res["result"]["serverInfo"]["name"], SERVER_NAME);
        assert!(res["result"]["capabilities"]["tools"].is_object());
        assert!(res["result"]["capabilities"]["prompts"].is_object());
    }

    #[test]
    fn ping_returns_empty_result() {
        let env = TestEnv::new();
        let res = send(&env, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#).expect("response");
        assert_eq!(res["id"], 2);
        assert_eq!(res["result"], json!({}));
    }

    #[test]
    fn notifications_get_no_response() {
        let env = TestEnv::new();
        assert!(send(
            &env,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .is_none());
        assert!(send(
            &env,
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
        )
        .is_none());
    }

    #[test]
    fn tools_list_contains_get_context_with_schema() {
        let env = TestEnv::new();
        let res =
            send(&env, r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#).expect("response");
        let tools = res["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], TOOL_GET_CONTEXT);
        let schema = &tools[0]["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["maxTokens"]["maximum"], 3000);
        assert_eq!(
            schema["properties"]["sensitivity"]["items"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn prompts_list_and_get() {
        let env = TestEnv::new();
        let res =
            send(&env, r#"{"jsonrpc":"2.0","id":4,"method":"prompts/list"}"#).expect("response");
        assert_eq!(res["result"]["prompts"][0]["name"], PROMPT_TASK_START);

        let res = send(
            &env,
            r#"{"jsonrpc":"2.0","id":5,"method":"prompts/get","params":{"name":"soul.task_start"}}"#,
        )
        .expect("response");
        assert!(res["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("soul.get_context"));
        assert!(res["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("voluntary"));

        let res = send(
            &env,
            r#"{"jsonrpc":"2.0","id":6,"method":"prompts/get","params":{"name":"nope"}}"#,
        )
        .expect("response");
        assert_eq!(res["error"]["code"], JSONRPC_INVALID_PARAMS);
    }

    #[test]
    fn get_context_returns_pack_metadata_and_writes_receipt() {
        let env = TestEnv::new();
        let res = send(
            &env,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"soul.get_context","arguments":{}}}"#,
        )
        .expect("response");
        assert!(res.get("error").is_none(), "unexpected error: {res}");
        let content = res["result"]["content"].as_array().unwrap();
        let serialized = content[0]["text"].as_str().unwrap();
        assert!(serialized.starts_with("SOUL CONTEXT"));
        assert!(serialized.contains("[") && serialized.contains("preference"));
        let metadata: Value = serde_json::from_str(content[1]["text"].as_str().unwrap()).unwrap();
        assert_eq!(metadata["entityCount"], 2);
        assert_eq!(metadata["policyVersion"], context::CONTEXT_POLICY_VERSION);
        assert!(metadata["stateVersion"].as_str().unwrap().len() == 8);

        // Квитанция записана и не содержит секретов.
        let receipts_dir = env.dir.join("receipts");
        assert!(receipts_dir.exists());
        let files: Vec<_> = std::fs::read_dir(&receipts_dir).unwrap().collect();
        assert_eq!(files.len(), 1);
        let path = files[0].as_ref().unwrap().path();
        let text = std::fs::read_to_string(path).unwrap();
        let receipt: DisclosureReceipt = serde_json::from_str(&text).unwrap();
        assert_eq!(receipt.kind, "disclosure");
        assert_eq!(receipt.entity_count, 2);
        assert!(!text.contains("Prefers concise answers"));
        assert!(!text.contains("Never share medical data"));
        assert!(!text.contains("ent_"));
    }

    #[test]
    fn get_context_respects_filters_and_budget() {
        let env = TestEnv::new();
        let call = |args: &str| {
            send(
                &env,
                &format!(
                    r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"soul.get_context","arguments":{args}}}}}"#
                ),
            )
        };
        // Текст отсекает нерелевантное: только preference.
        let res = call(r#"{"text":"concise"}"#).expect("response");
        let content = res["result"]["content"].as_array().unwrap();
        let serialized = content[0]["text"].as_str().unwrap();
        assert!(serialized.contains("Prefers concise answers"));
        assert!(!serialized.contains("Never share medical data"));

        // Чувствительность: sensitive исключается без запроса.
        let res = call(r#"{"sensitivity":["public","internal"]}"#).expect("response");
        let content = res["result"]["content"].as_array().unwrap();
        assert!(!content[0]["text"].as_str().unwrap().contains("Never share"));

        // Невалидные аргументы → isError, а не крах сервера.
        let res = call(r#"{"maxTokens":"big"}"#).expect("response");
        assert_eq!(res["result"]["isError"], true);
    }

    #[test]
    fn get_context_rejects_oversized_query_params() {
        let env = TestEnv::new();
        let call = |args: &str| {
            send(
                &env,
                &format!(
                    r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"soul.get_context","arguments":{args}}}}}"#
                ),
            )
        };
        let expect_err = |res: Option<Value>, needle: &str| {
            let res = res.expect("response");
            assert_eq!(res["result"]["isError"], true, "expected error: {res}");
            let text = res["result"]["content"][0]["text"].as_str().unwrap();
            assert!(text.contains(needle), "expected {needle} in: {text}");
        };

        let huge_text = format!(r#"{{"text":"{}"}}"#, "x".repeat(8001));
        expect_err(call(&huge_text), "too long");

        let huge_domains = format!(
            r#"{{"domains":{}}}"#,
            serde_json::to_string(&vec!["d".to_string(); 65]).unwrap()
        );
        expect_err(call(&huge_domains), "too large");

        let many_entries = format!(
            r#"{{"domains":{},"projects":{},"people":{},"channels":{}}}"#,
            serde_json::to_string(&vec!["d".to_string(); 64]).unwrap(),
            serde_json::to_string(&vec!["p".to_string(); 64]).unwrap(),
            serde_json::to_string(&vec!["o".to_string(); 64]).unwrap(),
            serde_json::to_string(&vec!["c".to_string(); 17]).unwrap()
        );
        expect_err(call(&many_entries), "total entries");

        let long_entry = format!(r#"{{"domains":["{}"]}}"#, "e".repeat(257));
        expect_err(call(&long_entry), "too long");

        let long_since = format!(r#"{{"since":"{}"}}"#, "s".repeat(65));
        expect_err(call(&long_since), "too long");
    }

    #[test]
    fn unknown_tool_and_method_are_errors() {
        let env = TestEnv::new();
        let res = send(
            &env,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"soul.other","arguments":{}}}"#,
        )
        .expect("response");
        assert_eq!(res["result"]["isError"], true);

        let res = send(&env, r#"{"jsonrpc":"2.0","id":10,"method":"wat"}"#).expect("response");
        assert_eq!(res["error"]["code"], JSONRPC_METHOD_NOT_FOUND);
    }

    #[test]
    fn malformed_json_and_bad_request_are_errors() {
        let env = TestEnv::new();
        let res = send(&env, "not json").expect("response");
        assert_eq!(res["error"]["code"], JSONRPC_PARSE_ERROR);

        let res = send(&env, r#"{"jsonrpc":"2.0","id":11}"#).expect("response");
        assert_eq!(res["error"]["code"], JSONRPC_INVALID_REQUEST);

        let res = send(&env, r#"{"id":12,"method":"ping"}"#).expect("response");
        assert_eq!(res["error"]["code"], JSONRPC_INVALID_REQUEST);
    }

    #[test]
    fn serve_io_roundtrips_initialize_and_call() {
        let env = TestEnv::new();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"soul.get_context\",\"arguments\":{}}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
        let mut reader = Cursor::new(input.as_bytes());
        let mut writer: Vec<u8> = Vec::new();
        serve_io(&mut reader, &mut writer, &env.dir).unwrap();
        let out = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "two responses for two requests: {out}");
        assert!(lines[0].contains("\"protocolVersion\""));
        assert!(lines[1].contains("SOUL CONTEXT"));
    }

    /// End-to-end: запускает настоящий бинарь soul-mcp.exe с SOUL_APP_DIR,
    /// гоняет initialize + tools/call по stdio и проверяет ответы. Если
    /// бинарь не собран (например, `cargo test --lib`) — тест пропускается.
    #[test]
    fn real_binary_serves_context_over_stdio() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let env = TestEnv::new();
        let exe = std::env::current_exe().unwrap();
        let mut bin = exe.parent().unwrap().parent().unwrap().to_path_buf();
        bin.push(if cfg!(windows) {
            "soul-mcp.exe"
        } else {
            "soul-mcp"
        });
        if !bin.exists() {
            eprintln!(
                "soul-mcp binary not built; skipping E2E test ({})",
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
            .expect("spawn soul-mcp");

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

        let mut request = |stdin: &mut std::process::ChildStdin, line: &str| {
            writeln!(stdin, "{line}").unwrap();
            stdin.flush().unwrap();
            let mut buf = String::new();
            stdout.read_line(&mut buf).unwrap();
            serde_json::from_str::<Value>(buf.trim()).unwrap()
        };

        let res = request(
            &mut stdin,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        );
        assert_eq!(res["id"], 1);
        assert_eq!(res["result"]["serverInfo"]["name"], SERVER_NAME);

        let res = request(
            &mut stdin,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"soul.get_context","arguments":{}}}"#,
        );
        assert!(res.get("error").is_none(), "unexpected error: {res}");
        let content = res["result"]["content"].as_array().unwrap();
        assert!(content[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("SOUL CONTEXT"));
        assert!(content[0]["text"].as_str().unwrap().contains("entities: 2"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap()
            .contains("Prefers concise answers"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap()
            .contains("Never share medical data"));

        // Квитанция disclosure появилась в каталоге.
        let receipts_dir = env.dir.join("receipts");
        let files: Vec<_> = std::fs::read_dir(&receipts_dir).unwrap().collect();
        assert_eq!(files.len(), 1);

        drop(stdin);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "soul-mcp exited with {status}");
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "soul-mcp did not exit on stdin EOF"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn empty_database_yields_empty_pack() {
        let dir = std::env::temp_dir().join(format!("soul-mcp-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init_db(&dir).unwrap(); // БД есть, душ нет
        let res = handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"soul.get_context","arguments":{}}}"#,
            &dir,
        )
        .expect("response");
        let content = res["result"]["content"].as_array().unwrap();
        assert!(content[0]["text"].as_str().unwrap().contains("entities: 0"));
        assert!(!content[0]["text"].as_str().unwrap().contains("CONFLICTS:"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_app_db_fails_when_database_missing() {
        let dir = std::env::temp_dir().join(format!("soul-mcp-nodb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(open_app_db(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_io_rejects_oversized_line_and_continues() {
        let env = TestEnv::new();
        let mut input = String::new();
        input.push('{');
        input.push_str(&"x".repeat(MCP_MAX_LINE_BYTES));
        input.push_str("\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n");
        let mut reader = Cursor::new(input.into_bytes());
        let mut writer: Vec<u8> = Vec::new();
        serve_io(&mut reader, &mut writer, &env.dir).unwrap();
        let out = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "oversized line must not kill the loop: {out}"
        );
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["error"]["code"], JSONRPC_PARSE_ERROR);
        assert!(lines[1].contains("\"protocolVersion\""));
    }

    #[test]
    fn read_only_connection_rejects_writes_even_after_fallback() {
        let dir = std::env::temp_dir().join(format!("soul-mcp-ro-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init_db(&dir).unwrap();
        let conn = open_app_db(&dir).unwrap();
        assert!(
            conn.execute_batch("DELETE FROM souls;").is_err(),
            "query_only must block writes after the read-write fallback"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
