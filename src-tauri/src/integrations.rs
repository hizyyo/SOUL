//! Интеграции с coding-клиентами: обнаружение (без скрытого наблюдения),
//! подключение/отключение с атомарной записью, backup, проверкой результата
//! и rollback. Модифицируются ТОЛЬКО три известных конфигурационных файла,
//! и только по явному действию пользователя в Settings.
//!
//! Гарантии:
//! - изменения атомарны (temp + rename), перед записью создаётся backup;
//! - каждая запись проверяется повторным чтением, при неудаче — rollback;
//! - отключение удаляет только запись SOUL и не затирает чужую конфигурацию:
//!   если файл менялся после подключения, восстанавливается НЕ backup, а
//!   выполняется точечное удаление нашей записи;
//! - нечитаемая чужая конфигурация не трогается (fail-closed).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientId {
    ClaudeCode,
    Codex,
    Cursor,
}

impl ClientId {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientId::ClaudeCode => "claude-code",
            ClientId::Codex => "codex",
            ClientId::Cursor => "cursor",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ClientId::ClaudeCode => "Claude Code",
            ClientId::Codex => "Codex",
            ClientId::Cursor => "Cursor",
        }
    }

    pub fn all() -> [ClientId; 3] {
        [ClientId::ClaudeCode, ClientId::Codex, ClientId::Cursor]
    }
}

impl std::str::FromStr for ClientId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude-code" => Ok(ClientId::ClaudeCode),
            "codex" => Ok(ClientId::Codex),
            "cursor" => Ok(ClientId::Cursor),
            other => Err(format!("Unknown client: {other}")),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ClientStatus {
    pub client: String,
    pub label: String,
    pub config_path: String,
    pub config_exists: bool,
    pub connected: bool,
    pub server_binary: String,
    pub server_binary_exists: bool,
    pub backup_path: Option<String>,
    pub error: Option<String>,
}

/// Состояние интеграции, записываемое локально (app_dir/integrations/<client>.json).
/// Используется для отключения и rollback; содержит только пути и хэши, без
/// содержимого чужой конфигурации.
#[derive(Debug, Serialize, Deserialize)]
pub struct IntegrationState {
    pub client: String,
    pub config_path: String,
    pub backup_path: String,
    pub original_hash: String,
    /// Хэш конфигурации ТОМУ, который мы записали при подключении: если
    /// текущий файл совпадает с ним, отключение восстанавливает backup.
    pub written_hash: String,
    pub connected_at: String,
}

pub fn home_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("USERPROFILE") {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    if let Ok(p) = std::env::var("HOME") {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    None
}

/// Имя исполняемого файла MCP-сервера (soul-mcp.exe на Windows).
pub fn mcp_bin_name() -> String {
    format!("soul-mcp{}", std::env::consts::EXE_SUFFIX)
}

/// Ожидаемый путь к MCP-бинарнику: рядом с текущим исполняемым файлом
/// (в dev — target/debug, в релизе — рядом с soul.exe).
pub fn server_binary_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|dir| dir.join(mcp_bin_name()))
        .unwrap_or_else(|| PathBuf::from(mcp_bin_name()))
}

pub fn client_config_path(client: ClientId, home: &Path) -> PathBuf {
    match client {
        ClientId::ClaudeCode => home.join(".claude.json"),
        ClientId::Codex => home.join(".codex").join("config.toml"),
        ClientId::Cursor => home.join(".cursor").join("mcp.json"),
    }
}

fn sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Атомарная запись: temp-файл в том же каталоге + rename.
fn atomic_write(target: &Path, content: &str) -> Result<(), String> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create config directory: {e}"))?;
    let name = target
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    let tmp = dir.join(format!(".{name}.soul-tmp-{}", Uuid::new_v4()));
    let replace_backup = replacement_backup_path(target);
    if !target.exists() && replace_backup.exists() {
        std::fs::rename(&replace_backup, target)
            .map_err(|e| format!("Cannot recover interrupted config replacement: {e}"))?;
    }
    std::fs::write(&tmp, content).map_err(|e| format!("Cannot write config: {e}"))?;
    match std::fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(first) if target.exists() => {
            let _ = std::fs::remove_file(&replace_backup);
            std::fs::rename(target, &replace_backup)
                .map_err(|e| format!("Cannot preserve config before replacement: {e}"))?;
            match std::fs::rename(&tmp, target) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&replace_backup);
                    Ok(())
                }
                Err(second) => {
                    let _ = std::fs::rename(&replace_backup, target);
                    let _ = std::fs::remove_file(&tmp);
                    Err(format!(
                        "Cannot replace config file: {second} (initial rename: {first})"
                    ))
                }
            }
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("Cannot replace config file: {e}"))
        }
    }
}

fn replacement_backup_path(target: &Path) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    dir.join(format!(".{name}.soul-replace-backup"))
}

fn recover_atomic_target(target: &Path) -> Result<(), String> {
    let backup = replacement_backup_path(target);
    if !target.exists() && backup.exists() {
        std::fs::rename(&backup, target)
            .map_err(|e| format!("Cannot recover interrupted config replacement: {e}"))?;
    } else if target.exists() && backup.exists() {
        std::fs::remove_file(&backup)
            .map_err(|e| format!("Cannot remove stale config replacement backup: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Чтение/проверка конфигураций
// ---------------------------------------------------------------------------

/// Команда MCP-сервера "soul" из JSON-конфигурации (Claude Code, Cursor).
/// Ok(None) = записи нет; Err = файл нельзя разобрать (fail-closed).
fn soul_command_json(config: &str) -> Result<Option<String>, String> {
    let trimmed = config.trim();
    let value: serde_json::Value = if trimmed.is_empty() {
        serde_json::Value::Object(Default::default())
    } else {
        serde_json::from_str(trimmed).map_err(|e| format!("config is not valid JSON: {e}"))?
    };
    let Some(obj) = value.as_object() else {
        return Err("config is not a JSON object".to_string());
    };
    let Some(servers) = obj.get("mcpServers").and_then(|s| s.as_object()) else {
        return Ok(None);
    };
    let Some(soul) = servers.get("soul") else {
        return Ok(None);
    };
    Ok(soul
        .get("command")
        .and_then(|c| c.as_str())
        .map(|c| c.to_string()))
}

/// Команда MCP-сервера "soul" из TOML-конфигурации (Codex: [mcp_servers.soul]).
fn soul_command_toml(config: &str) -> Result<Option<String>, String> {
    let value: toml::Value =
        toml::from_str(config).map_err(|e| format!("config is not valid TOML: {e}"))?;
    let Some(soul) = value.get("mcp_servers").and_then(|s| s.get("soul")) else {
        return Ok(None);
    };
    Ok(soul
        .get("command")
        .and_then(|c| c.as_str())
        .map(|c| c.to_string()))
}

fn soul_command(config: &str, client: ClientId) -> Result<Option<String>, String> {
    match client {
        ClientId::Codex => soul_command_toml(config),
        _ => soul_command_json(config),
    }
}

fn soul_capability(config: &str, client: ClientId) -> Result<Option<String>, String> {
    match client {
        ClientId::Codex => {
            let value: toml::Value =
                toml::from_str(config).map_err(|e| format!("config is not valid TOML: {e}"))?;
            Ok(value
                .get("mcp_servers")
                .and_then(|s| s.get("soul"))
                .and_then(|s| s.get("env"))
                .and_then(|env| env.get(crate::mcp::MCP_CAPABILITY_ENV))
                .and_then(|v| v.as_str())
                .map(str::to_string))
        }
        _ => {
            let value: serde_json::Value = serde_json::from_str(config)
                .map_err(|e| format!("config is not valid JSON: {e}"))?;
            Ok(value
                .get("mcpServers")
                .and_then(|s| s.get("soul"))
                .and_then(|s| s.get("env"))
                .and_then(|env| env.get(crate::mcp::MCP_CAPABILITY_ENV))
                .and_then(|v| v.as_str())
                .map(str::to_string))
        }
    }
}

/// Точечная вставка записи soul в JSON-конфигурацию (прочие ключи сохраняются).
fn insert_soul_entry_json(
    config: &str,
    binary: &str,
    app_dir: &Path,
    client: ClientId,
    capability: &str,
) -> Result<String, String> {
    let trimmed = config.trim();
    let mut value: serde_json::Value = if trimmed.is_empty() {
        serde_json::Value::Object(Default::default())
    } else {
        serde_json::from_str(trimmed).map_err(|e| format!("config is not valid JSON: {e}"))?
    };
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "config is not a JSON object".to_string())?;
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| "mcpServers is not a JSON object".to_string())?;
    servers.insert(
        "soul".to_string(),
        serde_json::json!({
            "command": binary,
            "args": [],
            "env": {
                "SOUL_APP_DIR": app_dir.to_string_lossy(),
                "SOUL_MCP_CLIENT": client.as_str(),
                crate::mcp::MCP_CAPABILITY_ENV: capability
            }
        }),
    );
    serde_json::to_string_pretty(&value).map_err(|e| format!("Cannot serialize config: {e}"))
}

/// Точечная вставка записи soul в TOML-конфигурацию Codex: добавляется
/// только секция [mcp_servers.soul] в конец файла — остальное содержимое
/// не переписывается (комментарии и форматирование пользователя сохраняются).
fn insert_soul_entry_toml(
    config: &str,
    binary: &str,
    app_dir: &Path,
    client: ClientId,
    capability: &str,
) -> Result<String, String> {
    let mut out = config.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!(
        "\n[mcp_servers.soul]\ncommand = '{}'\nargs = []\nenv = {{ SOUL_APP_DIR = '{}', SOUL_MCP_CLIENT = '{}', {} = '{}' }}\n",
        binary.replace('\'', "''"),
        app_dir.to_string_lossy().replace('\'', "''"),
        client.as_str(),
        crate::mcp::MCP_CAPABILITY_ENV,
        capability
    ));
    // Проверяем, что результат — валидный TOML с нашей записью (fail-closed).
    let value: toml::Value =
        toml::from_str(&out).map_err(|e| format!("resulting config is not valid TOML: {e}"))?;
    let Some(cmd) = value
        .get("mcp_servers")
        .and_then(|s| s.get("soul"))
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
    else {
        return Err("soul entry missing after TOML edit".to_string());
    };
    if cmd != binary {
        return Err("soul entry command mismatch after TOML edit".to_string());
    }
    Ok(out)
}

/// Удаление ТОЛЬКО записи soul из JSON (прочие серверы и ключи сохраняются).
fn remove_soul_entry_json(config: &str) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(config).map_err(|e| format!("config is not valid JSON: {e}"))?;
    let Some(obj) = value.as_object_mut() else {
        return Err("config is not a JSON object".to_string());
    };
    let Some(servers) = obj.get_mut("mcpServers").and_then(|s| s.as_object_mut()) else {
        return Ok(config.to_string());
    };
    servers.remove("soul");
    serde_json::to_string_pretty(&value).map_err(|e| format!("Cannot serialize config: {e}"))
}

/// Удаление ТОЛЬКО секции [mcp_servers.soul] из TOML (строки секции до
/// следующего заголовка). Остальное содержимое не переписывается.
fn remove_soul_entry_toml(config: &str) -> Result<String, String> {
    let lines: Vec<&str> = config.lines().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in lines {
        if skipping {
            if line.trim_start().starts_with('[') {
                skipping = false;
            } else {
                continue;
            }
        }
        if line.trim() == "[mcp_servers.soul]" {
            skipping = true;
            continue;
        }
        out.push(line);
    }
    let mut result = out.join("\n");
    while result.ends_with("\n\n") {
        result.pop();
    }
    let value: toml::Value =
        toml::from_str(&result).map_err(|e| format!("resulting config is not valid TOML: {e}"))?;
    if value
        .get("mcp_servers")
        .and_then(|s| s.get("soul"))
        .is_some()
    {
        return Err("soul entry still present after TOML edit".to_string());
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Состояние интеграции (локальный файл)
// ---------------------------------------------------------------------------

fn state_path(app_dir: &Path, client: ClientId) -> PathBuf {
    app_dir
        .join("integrations")
        .join(format!("{}.json", client.as_str()))
}

fn read_state(app_dir: &Path, client: ClientId) -> Option<IntegrationState> {
    let path = state_path(app_dir, client);
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_state(app_dir: &Path, state: &IntegrationState) -> Result<(), String> {
    let client = state
        .client
        .parse::<ClientId>()
        .map_err(|e| format!("Cannot persist integration state: {e}"))?;
    let path = state_path(app_dir, client);
    atomic_write(
        &path,
        &serde_json::to_string_pretty(state).map_err(|e| e.to_string())?,
    )
}

fn remove_state(app_dir: &Path, client: ClientId) {
    let _ = std::fs::remove_file(state_path(app_dir, client));
}

// ---------------------------------------------------------------------------
// Обнаружение
// ---------------------------------------------------------------------------

fn build_status(
    client: ClientId,
    config_path: &Path,
    binary: &Path,
    connected: bool,
    error: Option<String>,
    backup_path: Option<String>,
) -> ClientStatus {
    ClientStatus {
        client: client.as_str().to_string(),
        label: client.label().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        config_exists: config_path.exists(),
        connected,
        server_binary: binary.to_string_lossy().to_string(),
        server_binary_exists: binary.exists(),
        backup_path,
        error,
    }
}

/// Обнаружение: проверяет только существование и содержимое трёх известных
/// конфигурационных файлов (никаких логов, истории или других каталогов).
pub fn detect_clients(app_dir: &Path, binary: &Path) -> Vec<ClientStatus> {
    let Some(home) = home_dir() else {
        return ClientId::all()
            .iter()
            .map(|c| {
                build_status(
                    *c,
                    &PathBuf::new(),
                    binary,
                    false,
                    Some("Home directory not found".into()),
                    None,
                )
            })
            .collect();
    };
    detect_clients_for(&home, app_dir, binary)
}

/// Тестируемый вариант с явным home-каталогом.
pub fn detect_clients_for(home: &Path, app_dir: &Path, binary: &Path) -> Vec<ClientStatus> {
    ClientId::all()
        .iter()
        .map(|c| detect_client(app_dir, binary, *c, home))
        .collect()
}

fn detect_client(app_dir: &Path, binary: &Path, client: ClientId, home: &Path) -> ClientStatus {
    let config_path = client_config_path(client, home);
    let backup_path = read_state(app_dir, client).map(|s| s.backup_path);
    if let Err(e) = recover_atomic_target(&config_path) {
        return build_status(client, &config_path, binary, false, Some(e), backup_path);
    }
    let (connected, error) = if !config_path.exists() {
        (false, None)
    } else {
        match std::fs::read_to_string(&config_path) {
            Ok(text) => match soul_command(&text, client) {
                Ok(None) => (false, None),
                Ok(Some(cmd)) => {
                    let capability_ok = crate::crypto::ensure_local_capability_secret(app_dir)
                        .ok()
                        .and_then(|expected| {
                            soul_capability(&text, client)
                                .ok()
                                .flatten()
                                .map(|actual| actual == expected)
                        })
                        .unwrap_or(false);
                    if cmd == binary.to_string_lossy() && capability_ok {
                        (true, None)
                    } else {
                        (
                            false,
                            Some(
                                if cmd != binary.to_string_lossy() {
                                    "soul entry exists but points to a different command"
                                } else {
                                    "soul entry is missing valid local capability authorization"
                                }
                                .into(),
                            ),
                        )
                    }
                }
                Err(e) => (false, Some(e)),
            },
            Err(e) => (false, Some(format!("Cannot read config: {e}"))),
        }
    };
    build_status(client, &config_path, binary, connected, error, backup_path)
}

// ---------------------------------------------------------------------------
// Подключение / отключение / rollback
// ---------------------------------------------------------------------------

/// Подключение: backup → точечное изменение → атомарная запись → проверка →
/// rollback при неудаче → файл состояния.
pub fn connect_client(
    app_dir: &Path,
    binary: &Path,
    client: ClientId,
) -> Result<ClientStatus, String> {
    let home = home_dir().ok_or("Home directory not found.")?;
    connect_client_for(&home, app_dir, binary, client)
}

/// Тестируемый вариант с явным home-каталогом.
pub fn connect_client_for(
    home: &Path,
    app_dir: &Path,
    binary: &Path,
    client: ClientId,
) -> Result<ClientStatus, String> {
    let config_path = client_config_path(client, home);
    recover_atomic_target(&config_path)?;

    // Idempotентность: уже подключено этим же бинарником — просто статус.
    let original = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot read config: {e}"))?
    } else {
        String::new()
    };
    let mut working = original.clone();
    let capability = crate::crypto::ensure_local_capability_secret(app_dir)?;
    match soul_command(&working, client) {
        Ok(Some(cmd)) if cmd == binary.to_string_lossy() => {
            if soul_capability(&working, client)?.as_deref() == Some(capability.as_str()) {
                return Ok(detect_client(app_dir, binary, client, home));
            }
            working = match client {
                ClientId::Codex => remove_soul_entry_toml(&working)?,
                _ => remove_soul_entry_json(&working)?,
            };
        }
        Ok(Some(_)) => {
            return Err(
                "This client already has a soul entry pointing to a different command. Refusing to overwrite it."
                    .to_string(),
            );
        }
        Err(e) => return Err(format!("Refusing to modify the config: {e}")),
        Ok(None) => {}
    }

    // Backup точного исходного файла (даже если он отсутствовал — пустой backup
    // фиксирует «было пусто» и позволяет restore при отключении).
    let dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let name = config_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    let backup_path = dir.join(format!(
        "{name}.soul-backup-{}-{}",
        Uuid::new_v4(),
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    // Каталог конфига может отсутствовать (Cursor без ~/.cursor, Codex без
    // ~/.codex) — создаём его до записи backup, иначе connect падает раньше
    // атомарной записи, которая создала бы каталог сама.
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create config directory: {e}"))?;
    std::fs::write(&backup_path, &original).map_err(|e| format!("Cannot write backup: {e}"))?;

    let modified = match client {
        ClientId::Codex => insert_soul_entry_toml(
            &working,
            &binary.to_string_lossy(),
            app_dir,
            client,
            &capability,
        ),
        _ => insert_soul_entry_json(
            &working,
            &binary.to_string_lossy(),
            app_dir,
            client,
            &capability,
        ),
    }?;
    atomic_write(&config_path, &modified)?;

    // Проверка результата: запись реально применилась.
    let read_back =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot verify config: {e}"))?;
    let rollback_connect = || {
        if original.is_empty() {
            // Файла до подключения не было — не оставляем пустышку.
            let _ = std::fs::remove_file(&config_path);
        } else {
            let _ = atomic_write(&config_path, &original);
        }
    };
    match soul_command(&read_back, client) {
        Ok(Some(cmd)) if cmd == binary.to_string_lossy() => {}
        Ok(Some(_)) => {
            rollback_connect();
            return Err("Verification failed after connect; config rolled back.".to_string());
        }
        Ok(None) => {
            rollback_connect();
            return Err("soul entry missing after connect; config rolled back.".to_string());
        }
        Err(e) => {
            rollback_connect();
            return Err(format!("Config unreadable after connect; rolled back: {e}"));
        }
    }

    let state = IntegrationState {
        client: client.as_str().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        original_hash: sha256(&original),
        written_hash: sha256(&modified),
        connected_at: chrono::Utc::now().to_rfc3339(),
    };
    // Состояние — последний шаг: если его не удалось записать, откатываем
    // конфиг (иначе клиент «подключён», а UI не знает об этом) и убираем backup.
    if let Err(e) = write_state(app_dir, &state) {
        rollback_connect();
        let _ = std::fs::remove_file(&backup_path);
        return Err(format!(
            "Cannot persist integration state; config rolled back: {e}"
        ));
    }

    Ok(detect_client(app_dir, binary, client, home))
}

/// Removes all SOUL-managed MCP entries that can be safely parsed, then drops
/// SOUL's integration state and backups. Callers rotate the capability secret
/// first, so a malformed config that cannot be edited cannot retain access.
pub fn disconnect_all_after_capability_revocation(app_dir: &Path) {
    let Some(home) = home_dir() else {
        remove_all_states_and_backups(app_dir);
        return;
    };
    disconnect_all_after_capability_revocation_for(&home, app_dir);
}

fn disconnect_all_after_capability_revocation_for(home: &Path, app_dir: &Path) {
    for client in ClientId::all() {
        let _ = disconnect_client_for(home, app_dir, client);
    }
    remove_all_states_and_backups(app_dir);
}

fn remove_all_states_and_backups(app_dir: &Path) {
    for client in ClientId::all() {
        if let Some(state) = read_state(app_dir, client) {
            let _ = std::fs::remove_file(state.backup_path);
        }
        remove_state(app_dir, client);
    }
    let integrations_dir = app_dir.join("integrations");
    let _ = std::fs::remove_dir(&integrations_dir);
}

/// Отключение: если файл не менялся с момента подключения — восстанавливается
/// backup; если менялся (пользователь/клиент добавили своё) — удаляется только
/// запись SOUL, чужие изменения сохраняются. Проверка после каждого шага.
pub fn disconnect_client(app_dir: &Path, client: ClientId) -> Result<ClientStatus, String> {
    let home = home_dir().ok_or("Home directory not found.")?;
    disconnect_client_for(&home, app_dir, client)
}

/// Тестируемый вариант с явным home-каталогом.
pub fn disconnect_client_for(
    home: &Path,
    app_dir: &Path,
    client: ClientId,
) -> Result<ClientStatus, String> {
    let state = read_state(app_dir, client)
        .ok_or("This client was not connected by SOUL. Nothing to disconnect.".to_string())?;
    let config_path = PathBuf::from(&state.config_path);
    recover_atomic_target(&config_path)?;
    let current =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot read config: {e}"))?;

    // Файл не менялся с момента подключения (совпадает с тем, что мы записали)
    // → восстанавливаем backup. Если пользователь/клиент что-то изменили —
    // хирургически убираем только запись SOUL.
    let restored = if sha256(&current) == state.written_hash {
        if state.original_hash == sha256("") {
            // Файла до подключения не было — убираем его целиком. Проверка
            // ниже для этого случая: файла нет, записи soul нет.
            std::fs::remove_file(&config_path).map_err(|e| format!("Cannot remove config: {e}"))?;
            if config_path.exists() {
                return Err("Config still present after disconnect. Use Rollback.".to_string());
            }
            remove_state(app_dir, client);
            return Ok(detect_client(app_dir, &server_binary_path(), client, home));
        } else {
            let original = std::fs::read_to_string(&state.backup_path)
                .map_err(|e| format!("Cannot read backup: {e}"))?;
            atomic_write(&config_path, &original)?;
        }
        true
    } else {
        let cleaned = match client {
            ClientId::Codex => remove_soul_entry_toml(&current)?,
            _ => remove_soul_entry_json(&current)?,
        };
        atomic_write(&config_path, &cleaned)?;
        false
    };

    // Проверка: записи soul больше нет, файл читается.
    let read_back =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot verify config: {e}"))?;
    match soul_command(&read_back, client) {
        Ok(None) => {}
        Ok(Some(_)) => {
            return Err(
                "soul entry still present after disconnect; config left untouched — use Rollback."
                    .to_string(),
            );
        }
        Err(e) => {
            return Err(format!(
                "Config unreadable after disconnect (backup restored: {restored}); config left untouched — use Rollback: {e}"
            ));
        }
    }

    remove_state(app_dir, client);
    Ok(detect_client(app_dir, &server_binary_path(), client, home))
}

/// Ручной rollback: восстановление backup поверх текущего файла.
/// Явное действие пользователя, поэтому выполняется безусловно.
pub fn rollback_client(app_dir: &Path, client: ClientId) -> Result<ClientStatus, String> {
    let home = home_dir().ok_or("Home directory not found.")?;
    rollback_client_for(&home, app_dir, client)
}

/// Тестируемый вариант с явным home-каталогом.
pub fn rollback_client_for(
    home: &Path,
    app_dir: &Path,
    client: ClientId,
) -> Result<ClientStatus, String> {
    let state = read_state(app_dir, client)
        .ok_or("No rollback state found for this client.".to_string())?;
    let config_path = PathBuf::from(&state.config_path);
    recover_atomic_target(&config_path)?;
    let original = std::fs::read_to_string(&state.backup_path)
        .map_err(|e| format!("Cannot read backup: {e}"))?;
    if state.original_hash == sha256("") {
        // Файла до подключения не было — rollback убирает его, а не создаёт
        // пустышку (как при отключении).
        std::fs::remove_file(&config_path).map_err(|e| format!("Cannot remove config: {e}"))?;
    } else {
        atomic_write(&config_path, &original)?;
    }

    // Проверка после rollback: файла нет (был создан нами) или читается и
    // содержит запись, которую можно разобрать.
    if state.original_hash == sha256("") {
        if config_path.exists() {
            return Err("Config still present after rollback.".to_string());
        }
    } else {
        let read_back = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot verify config: {e}"))?;
        soul_command(&read_back, client)
            .map_err(|e| format!("Rollback result is unreadable: {e}"))?;
    }

    remove_state(app_dir, client);
    Ok(detect_client(app_dir, &server_binary_path(), client, home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestEnv {
        home: PathBuf,
        app_dir: PathBuf,
        binary: PathBuf,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let root = std::env::temp_dir().join(format!("soul-integ-test-{}", Uuid::new_v4()));
            let home = root.join("home");
            let app_dir = root.join("app");
            fs::create_dir_all(&home).unwrap();
            fs::create_dir_all(&app_dir).unwrap();
            TestEnv {
                home,
                app_dir,
                binary: root.join("soul-mcp.exe"),
            }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.home.parent().unwrap());
        }
    }

    fn binary_str(env: &TestEnv) -> String {
        env.binary.to_string_lossy().to_string()
    }

    fn json_cfg(env: &TestEnv, content: &str) -> PathBuf {
        let p = client_config_path(ClientId::ClaudeCode, &env.home);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        p
    }

    fn toml_cfg(env: &TestEnv, content: &str) -> PathBuf {
        let p = client_config_path(ClientId::Codex, &env.home);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        p
    }

    fn statuses(env: &TestEnv) -> Vec<ClientStatus> {
        detect_clients_for(&env.home, &env.app_dir, &env.binary)
    }

    fn status(env: &TestEnv, client: ClientId) -> ClientStatus {
        statuses(env)
            .into_iter()
            .find(|s| s.client == client.as_str())
            .unwrap()
    }

    fn connected(env: &TestEnv, client: ClientId) -> bool {
        status(env, client).connected
    }

    fn connect(env: &TestEnv, client: ClientId) -> Result<ClientStatus, String> {
        connect_client_for(&env.home, &env.app_dir, &env.binary, client)
    }

    fn disconnect(env: &TestEnv, client: ClientId) -> Result<ClientStatus, String> {
        disconnect_client_for(&env.home, &env.app_dir, client)
    }

    fn rollback(env: &TestEnv, client: ClientId) -> Result<ClientStatus, String> {
        rollback_client_for(&env.home, &env.app_dir, client)
    }

    #[test]
    fn detection_requires_only_config_file_presence() {
        let env = TestEnv::new();
        assert!(!connected(&env, ClientId::ClaudeCode));
        assert!(!connected(&env, ClientId::Codex));
        assert!(!connected(&env, ClientId::Cursor));
        assert_eq!(
            status(&env, ClientId::Cursor).config_path,
            env.home
                .join(".cursor")
                .join("mcp.json")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn connect_writes_entry_creates_backup_and_verifies() {
        let env = TestEnv::new();
        json_cfg(&env, r#"{ "mcpServers": { "other": { "command": "x" } } }"#);
        let s = connect(&env, ClientId::ClaudeCode).unwrap();
        assert!(s.connected);

        let text = fs::read_to_string(client_config_path(ClientId::ClaudeCode, &env.home)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["mcpServers"]["soul"]["command"], binary_str(&env));
        assert_eq!(
            v["mcpServers"]["soul"]["env"]["SOUL_APP_DIR"],
            env.app_dir.to_string_lossy().as_ref()
        );
        assert!(
            v["mcpServers"]["soul"]["env"][crate::mcp::MCP_CAPABILITY_ENV]
                .as_str()
                .is_some_and(|secret| secret.len() == 64)
        );
        assert_eq!(
            v["mcpServers"]["other"]["command"], "x",
            "чужая запись сохраняется"
        );

        // backup существует и содержит исходный файл.
        let backup = fs::read_to_string(s.backup_path.unwrap()).unwrap();
        assert!(backup.contains("\"other\""));
        assert!(!backup.contains("\"soul\""));

        // Файл состояния записан.
        assert!(state_path(&env.app_dir, ClientId::ClaudeCode).exists());
    }

    #[test]
    fn connect_is_idempotent_and_refuses_foreign_soul_entry() {
        let env = TestEnv::new();
        json_cfg(
            &env,
            r#"{ "mcpServers": { "soul": { "command": "C:\\other\\soul-mcp.exe" } } }"#,
        );
        let err = connect(&env, ClientId::ClaudeCode).unwrap_err();
        assert!(err.contains("different command"), "unexpected error: {err}");

        let env2 = TestEnv::new();
        let cfg = format!(
            "{{ \"mcpServers\": {{ \"soul\": {{ \"command\": \"{}\" }} }} }}",
            binary_str(&env2).replace('\\', "\\\\")
        );
        json_cfg(&env2, &cfg);
        connect(&env2, ClientId::ClaudeCode).unwrap();
        // Повторный connect — без ошибок и без дублей.
        connect(&env2, ClientId::ClaudeCode).unwrap();
        let text =
            fs::read_to_string(client_config_path(ClientId::ClaudeCode, &env2.home)).unwrap();
        assert_eq!(text.matches("\"soul\"").count(), 1);
    }

    #[test]
    fn capability_upgrade_rolls_back_to_the_byte_exact_original_config() {
        let env = TestEnv::new();
        let original = format!(
            "{{\n  \"mcpServers\": {{\n    \"soul\": {{ \"command\": \"{}\" }}\n  }}\n}}\n",
            binary_str(&env).replace('\\', "\\\\")
        );
        let path = json_cfg(&env, &original);

        connect(&env, ClientId::ClaudeCode).unwrap();
        rollback(&env, ClientId::ClaudeCode).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn deletion_cleanup_removes_all_managed_client_entries_and_state() {
        let env = TestEnv::new();
        connect(&env, ClientId::ClaudeCode).unwrap();
        connect(&env, ClientId::Cursor).unwrap();

        disconnect_all_after_capability_revocation_for(&env.home, &env.app_dir);

        assert!(!client_config_path(ClientId::ClaudeCode, &env.home).exists());
        assert!(!client_config_path(ClientId::Cursor, &env.home).exists());
        assert!(!state_path(&env.app_dir, ClientId::ClaudeCode).exists());
        assert!(!state_path(&env.app_dir, ClientId::Cursor).exists());
    }

    #[test]
    fn connect_creates_missing_config() {
        let env = TestEnv::new();
        connect(&env, ClientId::ClaudeCode).unwrap();
        let p = client_config_path(ClientId::ClaudeCode, &env.home);
        assert!(p.exists());
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["soul"]["command"], binary_str(&env));
    }

    #[test]
    fn connect_recovers_interrupted_config_replacement_before_reading() {
        let env = TestEnv::new();
        let path = client_config_path(ClientId::ClaudeCode, &env.home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let replacement_backup = replacement_backup_path(&path);
        fs::write(
            &replacement_backup,
            r#"{ "mcpServers": { "other": { "command": "x" } } }"#,
        )
        .unwrap();

        connect(&env, ClientId::ClaudeCode).unwrap();

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcpServers"]["other"]["command"], "x");
        assert_eq!(value["mcpServers"]["soul"]["command"], binary_str(&env));
        assert!(!replacement_backup.exists());
    }

    #[test]
    fn connect_creates_missing_config_directory() {
        // Cursor/Codex: каталог конфига (~/.cursor, ~/.codex) может отсутствовать
        // целиком — connect должен создавать его, а не падать на backup.
        let env = TestEnv::new();
        connect(&env, ClientId::Cursor).unwrap();
        let p = client_config_path(ClientId::Cursor, &env.home);
        assert!(p.exists());
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["soul"]["command"], binary_str(&env));

        let env2 = TestEnv::new();
        connect(&env2, ClientId::Codex).unwrap();
        let p = client_config_path(ClientId::Codex, &env2.home);
        assert!(p.exists());
        let text = fs::read_to_string(p).unwrap();
        assert!(text.contains("[mcp_servers.soul]"));
        assert!(text.contains(crate::mcp::MCP_CAPABILITY_ENV));
    }

    #[test]
    fn disconnect_removes_config_that_did_not_exist_before() {
        // Файла до подключения не было — disconnect убирает файл целиком и
        // не ошибается на повторной проверке (регрессия: раньше падал с
        // "Cannot verify config").
        let env = TestEnv::new();
        connect(&env, ClientId::Cursor).unwrap();
        assert!(connected(&env, ClientId::Cursor));

        disconnect(&env, ClientId::Cursor).unwrap();
        let p = client_config_path(ClientId::Cursor, &env.home);
        assert!(!p.exists(), "конфиг удалён целиком");
        assert!(!state_path(&env.app_dir, ClientId::Cursor).exists());
        assert!(!connected(&env, ClientId::Cursor));
    }

    #[test]
    fn connect_fails_closed_on_invalid_config() {
        let env = TestEnv::new();
        let p = json_cfg(&env, "not json at all");
        let err = connect(&env, ClientId::ClaudeCode).unwrap_err();
        assert!(err.contains("Refusing"), "unexpected error: {err}");
        // Файл не тронут.
        assert_eq!(fs::read_to_string(p).unwrap(), "not json at all");
        assert!(!state_path(&env.app_dir, ClientId::ClaudeCode).exists());
    }

    #[test]
    fn disconnect_restores_backup_when_file_unchanged() {
        let env = TestEnv::new();
        let original = r#"{ "mcpServers": { "other": { "command": "x" } } }"#;
        json_cfg(&env, original);
        connect(&env, ClientId::ClaudeCode).unwrap();
        assert!(connected(&env, ClientId::ClaudeCode));

        disconnect(&env, ClientId::ClaudeCode).unwrap();
        assert!(!connected(&env, ClientId::ClaudeCode));
        assert_eq!(
            fs::read_to_string(client_config_path(ClientId::ClaudeCode, &env.home)).unwrap(),
            original
        );
        assert!(!state_path(&env.app_dir, ClientId::ClaudeCode).exists());
    }

    #[test]
    fn disconnect_surgically_removes_only_soul_when_file_changed() {
        let env = TestEnv::new();
        json_cfg(&env, r#"{ "mcpServers": { "other": { "command": "x" } } }"#);
        connect(&env, ClientId::ClaudeCode).unwrap();

        // Пользователь добавил свой сервер после подключения.
        let p = client_config_path(ClientId::ClaudeCode, &env.home);
        let mut v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        v["mcpServers"]["user-added"] = serde_json::json!({ "command": "y" });
        fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();

        disconnect(&env, ClientId::ClaudeCode).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            v["mcpServers"].get("soul").is_none(),
            "soul удаляется: {text}"
        );
        assert_eq!(
            v["mcpServers"]["user-added"]["command"], "y",
            "чужие изменения сохраняются"
        );
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
    }

    #[test]
    fn rollback_removes_config_that_did_not_exist_before() {
        // Файла до подключения не было — rollback не должен оставлять пустышку.
        let env = TestEnv::new();
        connect(&env, ClientId::Cursor).unwrap();
        let p = client_config_path(ClientId::Cursor, &env.home);
        assert!(p.exists());

        rollback(&env, ClientId::Cursor).unwrap();
        assert!(!p.exists(), "файл убран, а не оставлен пустым");
        assert!(!state_path(&env.app_dir, ClientId::Cursor).exists());
        assert!(!connected(&env, ClientId::Cursor));
    }

    #[test]
    fn rollback_restores_backup() {
        let env = TestEnv::new();
        let original = r#"{ "mcpServers": { "other": { "command": "x" } } }"#;
        json_cfg(&env, original);
        connect(&env, ClientId::ClaudeCode).unwrap();

        // Намеренно испортим файл, чтобы проверить безусловный rollback.
        let p = client_config_path(ClientId::ClaudeCode, &env.home);
        fs::write(&p, "broken").unwrap();
        rollback(&env, ClientId::ClaudeCode).unwrap();
        assert_eq!(fs::read_to_string(p).unwrap(), original);
    }

    #[test]
    fn codex_toml_connect_and_disconnect_preserve_content() {
        let env = TestEnv::new();
        let original = "# user comment\n[model]\nprovider = \"openai\"\n";
        toml_cfg(&env, original);
        connect(&env, ClientId::Codex).unwrap();
        assert!(connected(&env, ClientId::Codex));

        let p = client_config_path(ClientId::Codex, &env.home);
        let text = fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[mcp_servers.soul]"),
            "секция добавлена: {text}"
        );
        assert!(text.contains(&format!("command = '{}'", binary_str(&env))));
        assert!(
            text.contains("[model]"),
            "чужое содержимое не тронуто: {text}"
        );
        assert!(text.contains("provider = \"openai\""));

        disconnect(&env, ClientId::Codex).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(!text.contains("mcp_servers.soul"), "секция удалена: {text}");
        assert!(text.contains("[model]"), "модель сохранена: {text}");
        assert!(!connected(&env, ClientId::Codex));
    }

    #[test]
    fn codex_toml_surgical_removal_keeps_other_mcp_servers() {
        let env = TestEnv::new();
        let original = "[mcp_servers.github]\ncommand = 'gh'\n";
        toml_cfg(&env, original);
        connect(&env, ClientId::Codex).unwrap();

        let p = client_config_path(ClientId::Codex, &env.home);
        fs::write(
            &p,
            fs::read_to_string(&p).unwrap() + "[mcp_servers.newone]\ncommand = 'x'\n",
        )
        .unwrap();

        disconnect(&env, ClientId::Codex).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(!text.contains("mcp_servers.soul"));
        assert!(text.contains("[mcp_servers.github]"));
        assert!(text.contains("[mcp_servers.newone]"));
    }
}
