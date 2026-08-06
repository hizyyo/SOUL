//! Регистрация Native Messaging host в браузерах Chromium (SESSION-09).
//!
//! Регистрация двухуровневая:
//! 1. манифест host-а `<app_dir>/native-messaging/com.soul.browser_companion.json`
//!    с путём к `soul-bridge(.exe)` и `allowed_origins` ровно для одного
//!    extension ID — браузер не даст подключиться другому расширению;
//! 2. Windows: ключи реестра HKCU для Chrome и Edge (через `reg.exe` — без
//!    новых крейтов). На других ОС регистрация явно возвращает unsupported,
//!    пока платформенные пути установки не реализованы и не квалифицированы.
//!
//! Команды в UI (register/unregister/status) используют чистые функции
//! `*_for(runner, ...)` — тесты подменяют runner и не трогают реальную систему.

use crate::bridge;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Output;

#[cfg(target_os = "windows")]
pub const CHROME_REG_SUBKEY: &str = r"Software\Google\Chrome\NativeMessagingHosts";
#[cfg(target_os = "windows")]
pub const EDGE_REG_SUBKEY: &str = r"Software\Microsoft\Edge\NativeMessagingHosts";
pub const HOST_MANIFEST_DIR_NAME: &str = "native-messaging";
#[cfg(not(target_os = "windows"))]
pub const UNSUPPORTED_NATIVE_HOST_MESSAGE: &str =
    "Browser Companion native-host registration is currently supported on Windows only.";

/// Выполнитель команд ОС. Прод — `reg.exe`; тесты подменяют фейком.
pub type CommandRunner = dyn Fn(&[&str]) -> Result<Output, std::io::Error>;

/// Запуск `reg.exe` (только Windows).
#[cfg(target_os = "windows")]
pub fn run_reg(args: &[&str]) -> Result<Output, std::io::Error> {
    let mut command = std::process::Command::new("reg");
    command.args(args);
    command.output()
}

/// Заглушка runner для платформ без реестра (вызовы не производятся).
#[cfg(not(target_os = "windows"))]
fn noop_runner(_args: &[&str]) -> Result<Output, std::io::Error> {
    Err(std::io::Error::other(
        "registry is not available on this platform",
    ))
}

/// Имя исполняемого файла host-а (soul-bridge.exe на Windows).
pub fn bridge_bin_name() -> String {
    format!("soul-bridge{}", std::env::consts::EXE_SUFFIX)
}

/// Ожидаемый путь к host-бинарнику: рядом с текущим исполняемым файлом
/// (в dev — target/debug, в релизе — рядом с soul.exe).
pub fn bridge_binary_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|dir| dir.join(bridge_bin_name()))
        .unwrap_or_else(|| PathBuf::from(bridge_bin_name()))
}

/// Путь к манифесту host-а в каталоге приложения.
pub fn host_manifest_path(app_dir: &Path) -> PathBuf {
    app_dir
        .join(HOST_MANIFEST_DIR_NAME)
        .join(format!("{}.json", bridge::BRIDGE_HOST_NAME))
}

/// Содержимое манифеста host-а. `allowed_origins` ограничивает подключения
/// ровно одним extension ID.
pub fn host_manifest_json(binary_path: &Path, extension_id: &str) -> serde_json::Value {
    json!({
        "name": bridge::BRIDGE_HOST_NAME,
        "description": "SOUL Browser Companion bridge: provides local SOUL context to the extension on supported AI web chats.",
        "path": binary_path.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{extension_id}/")]
    })
}

/// Регистрация через реестр Windows для одного браузера.
#[cfg(target_os = "windows")]
fn register_registry_key(
    runner: &CommandRunner,
    subkey: &str,
    manifest_path: &str,
) -> Result<(), String> {
    let out = runner(&[
        "add",
        &format!(r"HKCU\{subkey}\{}", bridge::BRIDGE_HOST_NAME),
        "/ve",
        "/d",
        manifest_path,
        "/f",
    ])
    .map_err(|e| format!("Cannot run reg.exe: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "reg add failed for {subkey}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Удаление ключа реестра для одного браузера.
#[cfg(target_os = "windows")]
fn unregister_registry_key(runner: &CommandRunner, subkey: &str) -> Result<(), String> {
    let out = runner(&[
        "delete",
        &format!(r"HKCU\{subkey}\{}", bridge::BRIDGE_HOST_NAME),
        "/f",
    ])
    .map_err(|e| format!("Cannot run reg.exe: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "reg delete failed for {subkey}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Проверка наличия ключа реестра (exit code 0 = ключ есть).
#[cfg(target_os = "windows")]
fn registry_key_exists(runner: &CommandRunner, subkey: &str) -> bool {
    runner(&[
        "query",
        &format!(r"HKCU\{subkey}\{}", bridge::BRIDGE_HOST_NAME),
    ])
    .map(|out| out.status.success())
    .unwrap_or(false)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeStatus {
    pub host_name: String,
    pub registered: bool,
    pub manifest_path: String,
    pub manifest_exists: bool,
    pub binary_path: String,
    pub binary_exists: bool,
    pub extension_id: String,
    pub error: String,
}

/// Регистрирует host: пишет манифест и создаёт ключи реестра (Windows).
pub fn register_bridge(app_dir: &Path) -> Result<BridgeStatus, String> {
    #[cfg(target_os = "windows")]
    {
        register_bridge_for(app_dir, &run_reg)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_dir;
        Err(UNSUPPORTED_NATIVE_HOST_MESSAGE.to_string())
    }
}

#[cfg(target_os = "windows")]
pub fn register_bridge_for(app_dir: &Path, runner: &CommandRunner) -> Result<BridgeStatus, String> {
    let manifest_path = host_manifest_path(app_dir);
    std::fs::create_dir_all(manifest_path.parent().unwrap())
        .map_err(|e| format!("Cannot create native-messaging directory: {e}"))?;
    let binary = bridge_binary_path();
    let manifest = host_manifest_json(&binary, bridge::BRIDGE_EXTENSION_ID);
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let tmp = manifest_path.with_extension("json.tmp");
    std::fs::write(&tmp, &text).map_err(|e| format!("Cannot write host manifest: {e}"))?;
    std::fs::rename(&tmp, &manifest_path)
        .map_err(|e| format!("Cannot write host manifest: {e}"))?;

    let path_str = manifest_path.to_string_lossy().to_string();
    register_registry_key(runner, CHROME_REG_SUBKEY, &path_str)?;
    register_registry_key(runner, EDGE_REG_SUBKEY, &path_str)?;

    Ok(bridge_status_for(app_dir, runner))
}

#[cfg(not(target_os = "windows"))]
pub fn register_bridge_for(
    _app_dir: &Path,
    _runner: &CommandRunner,
) -> Result<BridgeStatus, String> {
    Err(UNSUPPORTED_NATIVE_HOST_MESSAGE.to_string())
}

/// Удаляет регистрацию: ключи реестра и манифест. Отсутствующие ключи не
/// считаются ошибкой (idempotent).
pub fn unregister_bridge(app_dir: &Path) -> Result<BridgeStatus, String> {
    #[cfg(target_os = "windows")]
    {
        unregister_bridge_for(app_dir, &run_reg)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_dir;
        Err(UNSUPPORTED_NATIVE_HOST_MESSAGE.to_string())
    }
}

#[cfg(target_os = "windows")]
pub fn unregister_bridge_for(
    app_dir: &Path,
    runner: &CommandRunner,
) -> Result<BridgeStatus, String> {
    let _ = unregister_registry_key(runner, CHROME_REG_SUBKEY);
    let _ = unregister_registry_key(runner, EDGE_REG_SUBKEY);
    let manifest_path = host_manifest_path(app_dir);
    if manifest_path.exists() {
        std::fs::remove_file(&manifest_path)
            .map_err(|e| format!("Cannot remove host manifest: {e}"))?;
    }
    Ok(bridge_status_for(app_dir, runner))
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_bridge_for(
    _app_dir: &Path,
    _runner: &CommandRunner,
) -> Result<BridgeStatus, String> {
    Err(UNSUPPORTED_NATIVE_HOST_MESSAGE.to_string())
}

/// Текущее состояние регистрации.
pub fn bridge_status(app_dir: &Path) -> BridgeStatus {
    #[cfg(target_os = "windows")]
    {
        bridge_status_for(app_dir, &run_reg)
    }
    #[cfg(not(target_os = "windows"))]
    {
        bridge_status_for(app_dir, &noop_runner)
    }
}

pub fn bridge_status_for(app_dir: &Path, runner: &CommandRunner) -> BridgeStatus {
    let manifest_path = host_manifest_path(app_dir);
    let binary = bridge_binary_path();
    let mut status = BridgeStatus {
        host_name: bridge::BRIDGE_HOST_NAME.to_string(),
        registered: false,
        manifest_path: manifest_path.to_string_lossy().to_string(),
        manifest_exists: manifest_path.exists(),
        binary_path: binary.to_string_lossy().to_string(),
        binary_exists: binary.exists(),
        extension_id: bridge::BRIDGE_EXTENSION_ID.to_string(),
        error: String::new(),
    };
    #[cfg(target_os = "windows")]
    {
        let chrome = registry_key_exists(runner, CHROME_REG_SUBKEY);
        let edge = registry_key_exists(runner, EDGE_REG_SUBKEY);
        status.registered = chrome || edge;
        if !chrome && !edge && status.manifest_exists {
            status.error = "Manifest exists but no browser registry key was found.".to_string();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = runner;
        status.registered = false;
        status.error = UNSUPPORTED_NATIVE_HOST_MESSAGE.to_string();
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt as _;
    #[cfg(target_os = "windows")]
    use std::os::windows::process::ExitStatusExt as _;

    /// Фейковый runner: записывает вызовы; `fail_contains` — если любой аргумент
    /// содержит подстроку, команда отвечает кодом 1. Замыкание 'static (Arc),
    /// чтобы не связывать жизнь runner-а и вызова.
    struct FakeRunner {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
        fail_contains: Option<String>,
    }

    impl FakeRunner {
        fn new() -> FakeRunner {
            FakeRunner {
                calls: Arc::new(Mutex::new(Vec::new())),
                fail_contains: None,
            }
        }

        fn command(&self) -> impl Fn(&[&str]) -> Result<Output, std::io::Error> + 'static {
            let calls = self.calls.clone();
            let fail_contains = self.fail_contains.clone();
            move |args: &[&str]| {
                calls
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                let failed = fail_contains
                    .as_ref()
                    .map(|needle| args.iter().any(|a| a.contains(needle)))
                    .unwrap_or(false);
                Ok(Output {
                    status: std::process::ExitStatus::from_raw(if failed { 1 } else { 0 }),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    struct TestEnv {
        dir: PathBuf,
    }

    impl TestEnv {
        fn new() -> TestEnv {
            let dir = std::env::temp_dir().join(format!("soul-nh-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            TestEnv { dir }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[cfg(target_os = "windows")]
    fn manifest_of(env: &TestEnv) -> serde_json::Value {
        let path = host_manifest_path(&env.dir);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn host_manifest_contains_name_type_path_and_allowed_origins() {
        let manifest = host_manifest_json(
            Path::new("C:/App/soul-bridge.exe"),
            bridge::BRIDGE_EXTENSION_ID,
        );
        assert_eq!(manifest["name"], bridge::BRIDGE_HOST_NAME);
        assert_eq!(manifest["type"], "stdio");
        assert_eq!(manifest["path"], "C:/App/soul-bridge.exe");
        assert_eq!(
            manifest["allowed_origins"],
            json!([format!(
                "chrome-extension://{}/",
                bridge::BRIDGE_EXTENSION_ID
            )])
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn register_writes_manifest_and_registry_keys() {
        let env = TestEnv::new();
        let runner = FakeRunner::new();
        let status = register_bridge_for(&env.dir, &runner.command()).unwrap();
        assert!(status.manifest_exists);
        assert!(status.binary_path.contains("soul-bridge"));

        let manifest = manifest_of(&env);
        assert_eq!(manifest["name"], bridge::BRIDGE_HOST_NAME);
        assert!(manifest["path"].as_str().unwrap().contains("soul-bridge"));

        let calls = runner.calls();
        let adds: Vec<&Vec<String>> = calls.iter().filter(|c| c[0] == "add").collect();
        assert_eq!(adds.len(), 2, "chrome + edge keys");
        assert!(adds
            .iter()
            .any(|c| c[1].contains("Google\\Chrome\\NativeMessagingHosts")));
        assert!(adds
            .iter()
            .any(|c| c[1].contains("Microsoft\\Edge\\NativeMessagingHosts")));
        for call in &adds {
            assert_eq!(call[2], "/ve");
            assert_eq!(call[5], "/f");
            assert!(call[4].ends_with("com.soul.browser_companion.json"));
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn unregister_removes_manifest_and_keys_idempotently() {
        let env = TestEnv::new();
        let runner = FakeRunner::new();
        let _ = register_bridge_for(&env.dir, &runner.command()).unwrap();
        let status = unregister_bridge_for(&env.dir, &runner.command()).unwrap();
        assert!(!status.manifest_exists);
        let deletes: Vec<Vec<String>> = runner
            .calls()
            .into_iter()
            .filter(|c| c[0] == "delete")
            .collect();
        assert_eq!(deletes.len(), 2);
        // Второй раз — тоже успех (idempotent).
        let status = unregister_bridge_for(&env.dir, &runner.command()).unwrap();
        assert!(!status.manifest_exists);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn status_reflects_manifest_and_registry() {
        let env = TestEnv::new();
        // reg query отвечает "ключа нет" → не зарегистрировано.
        let no_keys_runner = FakeRunner {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_contains: Some("query".to_string()),
        };
        let status = bridge_status_for(&env.dir, &no_keys_runner.command());
        assert!(!status.registered);
        assert!(!status.manifest_exists);
        assert_eq!(status.extension_id, bridge::BRIDGE_EXTENSION_ID);
        assert_eq!(status.host_name, bridge::BRIDGE_HOST_NAME);

        let ok_runner = FakeRunner::new();
        let _ = register_bridge_for(&env.dir, &ok_runner.command()).unwrap();
        let status = bridge_status_for(&env.dir, &ok_runner.command());
        assert!(status.registered);
        assert!(status.manifest_exists);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn status_flags_orphan_manifest_when_registry_missing() {
        let env = TestEnv::new();
        let runner = FakeRunner::new();
        let _ = register_bridge_for(&env.dir, &runner.command()).unwrap();
        // Теперь reg query отвечает "ключа нет".
        let runner2 = FakeRunner {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_contains: Some("NativeMessagingHosts".to_string()),
        };
        let status = bridge_status_for(&env.dir, &runner2.command());
        assert!(!status.registered);
        assert!(status.manifest_exists);
        assert!(status.error.contains("Manifest exists"));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn non_windows_registration_reports_unsupported_without_writing_manifest() {
        let env = TestEnv::new();
        let runner = FakeRunner::new();
        let error = register_bridge_for(&env.dir, &runner.command()).unwrap_err();
        assert_eq!(error, UNSUPPORTED_NATIVE_HOST_MESSAGE);
        assert!(!host_manifest_path(&env.dir).exists());
        assert!(runner.calls().is_empty());
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn non_windows_unregister_and_status_report_unsupported() {
        let env = TestEnv::new();
        let runner = FakeRunner::new();
        let error = unregister_bridge_for(&env.dir, &runner.command()).unwrap_err();
        assert_eq!(error, UNSUPPORTED_NATIVE_HOST_MESSAGE);

        let manifest_path = host_manifest_path(&env.dir);
        std::fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        std::fs::write(&manifest_path, "{}").unwrap();
        let status = bridge_status_for(&env.dir, &runner.command());
        assert!(!status.registered);
        assert!(status.manifest_exists);
        assert_eq!(status.error, UNSUPPORTED_NATIVE_HOST_MESSAGE);
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn manifest_path_lives_under_app_dir() {
        let env = TestEnv::new();
        let path = host_manifest_path(&env.dir);
        assert!(path.starts_with(&env.dir));
        assert_eq!(path.file_name().unwrap(), "com.soul.browser_companion.json");
    }
}
