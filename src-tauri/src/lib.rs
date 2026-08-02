pub mod bridge;
mod context;
mod crypto;
mod db;
mod eval;
mod gateway;
mod integrations;
mod native_host;
mod package;
mod policy;

pub mod mcp;

use db::{
    activate_preview, activate_soul, add_entity, confirm_soul_preview, create_soul,
    get_calibration, get_soul, init_db, is_soul_activated, list_entities, list_souls,
    reset_soul_preview, save_calibration, update_entity,
};
use package::{
    DeletionReceipt, ExportReceipt, ImportPreview, JsonExportReceipt, MarkdownExportReceipt,
    ReceiptSummary,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    conn: Mutex<rusqlite::Connection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SoulInfo {
    soul_id: String,
    display_name: String,
    format_version: String,
    schema_version: String,
    created_at: String,
    head_event_hash: Option<String>,
    entity_count: i64,
    device_id: String,
    activated: bool,
    calibration_step: i32,
    preview_confirmed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityInfo {
    id: String,
    soul_id: String,
    entity_type: String,
    status: String,
    data: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CalibrationState {
    step: i32,
    answers: String,
}

fn soul_to_info(conn: &rusqlite::Connection, s: &db::SoulManifest) -> SoulInfo {
    let activated = is_soul_activated(conn, &s.soul_id).unwrap_or(false);
    let (cstep, _, _, preview_confirmed) =
        db::get_soul_state(conn, &s.soul_id).unwrap_or((0, "[]".to_string(), false, false));
    SoulInfo {
        soul_id: s.soul_id.clone(),
        display_name: s.display_name.clone(),
        format_version: s.format_version.clone(),
        schema_version: s.schema_version.clone(),
        created_at: s.created_at.clone(),
        head_event_hash: s.head_event_hash.clone(),
        entity_count: s.entity_count,
        device_id: s.device_id.clone(),
        activated,
        calibration_step: cstep,
        preview_confirmed,
    }
}

fn entity_to_info(r: &db::EntityRow) -> EntityInfo {
    EntityInfo {
        id: r.id.clone(),
        soul_id: r.soul_id.clone(),
        entity_type: r.entity_type.clone(),
        status: r.status.clone(),
        data: r.data.clone(),
        created_at: r.created_at.clone(),
        updated_at: r.updated_at.clone(),
    }
}

#[tauri::command]
fn health() -> String {
    "ok".into()
}

#[tauri::command]
fn init_app(app: tauri::AppHandle) -> Result<SoulInfo, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

    let conn = init_db(&app_dir).map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    *state.conn.lock().map_err(|e| e.to_string())? = conn;

    let souls = list_souls_internal(&state).map_err(|e| e.to_string())?;
    if let Some(s) = souls.first() {
        let guard = state.conn.lock().map_err(|e| e.to_string())?;
        Ok(soul_to_info(&guard, s))
    } else {
        Err("No SOUL found. Create one first.".into())
    }
}

#[tauri::command]
fn create_soul_cmd(
    state: tauri::State<AppState>,
    display_name: String,
    device_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let manifest = create_soul(&conn, &display_name, &device_id).map_err(|e| e.to_string())?;
    Ok(soul_to_info(&conn, &manifest))
}

#[tauri::command]
fn get_soul_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
) -> Result<Option<SoulInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    match get_soul(&conn, &soul_id).map_err(|e| e.to_string())? {
        Some(s) => Ok(Some(soul_to_info(&conn, &s))),
        None => Ok(None),
    }
}

#[tauri::command]
fn add_entity_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    entity_type: String,
    status: String,
    data: String,
    device_id: String,
) -> Result<EntityInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let row = add_entity(&conn, &soul_id, &entity_type, &status, &data, &device_id)
        .map_err(|e| e.to_string())?;
    Ok(entity_to_info(&row))
}

#[tauri::command]
fn update_entity_cmd(
    state: tauri::State<AppState>,
    entity_id: String,
    status: String,
    data: Option<String>,
    device_id: String,
) -> Result<EntityInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let row = update_entity(&conn, &entity_id, &status, data.as_deref(), &device_id)?;
    Ok(entity_to_info(&row))
}

#[tauri::command]
fn list_entities_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
) -> Result<Vec<EntityInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = list_entities(&conn, &soul_id).map_err(|e| e.to_string())?;
    Ok(rows.iter().map(entity_to_info).collect())
}

#[tauri::command]
fn search_entities_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    query: String,
    limit: usize,
) -> Result<Vec<EntityInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = db::search_entities(&conn, &soul_id, &query, limit).map_err(|e| e.to_string())?;
    Ok(rows.iter().map(entity_to_info).collect())
}

#[tauri::command]
fn get_calibration_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
) -> Result<CalibrationState, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let (step, answers) = get_calibration(&conn, &soul_id).map_err(|e| e.to_string())?;
    Ok(CalibrationState { step, answers })
}

#[tauri::command]
fn save_calibration_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    step: i32,
    answers: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    save_calibration(&conn, &soul_id, step, &answers).map_err(|e| e.to_string())
}

#[tauri::command]
fn confirm_preview_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    device_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    confirm_soul_preview(&conn, &soul_id, &device_id).map_err(|e| e.to_string())?;
    let s = get_soul(&conn, &soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found".to_string())?;
    Ok(soul_to_info(&conn, &s))
}

#[tauri::command]
fn reset_preview_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    device_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    reset_soul_preview(&conn, &soul_id, &device_id).map_err(|e| e.to_string())?;
    let s = get_soul(&conn, &soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found".to_string())?;
    Ok(soul_to_info(&conn, &s))
}

#[tauri::command]
fn activate_soul_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    device_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    activate_soul(&conn, &soul_id, &device_id).map_err(|e| e.to_string())?;
    let s = get_soul(&conn, &soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found".to_string())?;
    Ok(soul_to_info(&conn, &s))
}

#[tauri::command]
fn activate_preview_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    entity_ids: Vec<String>,
    device_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    activate_preview(&conn, &soul_id, &entity_ids, &device_id).map_err(|e| e.to_string())?;
    let s = get_soul(&conn, &soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found".to_string())?;
    Ok(soul_to_info(&conn, &s))
}

fn list_souls_internal(state: &AppState) -> Result<Vec<db::SoulManifest>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    list_souls(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_soul_cmd(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    soul_id: String,
    password: String,
    path: String,
) -> Result<ExportReceipt, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    package::export_package(
        &conn,
        &app_dir,
        &soul_id,
        &password,
        std::path::Path::new(&path),
    )
}

#[tauri::command]
fn inspect_soul_file_cmd(path: String, password: String) -> Result<ImportPreview, String> {
    package::inspect_package_file(std::path::Path::new(&path), &password)
}

#[tauri::command]
fn import_soul_file_cmd(
    state: tauri::State<AppState>,
    path: String,
    password: String,
) -> Result<SoulInfo, String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    let manifest = package::import_package_file(&mut conn, std::path::Path::new(&path), &password)?;
    Ok(soul_to_info(&conn, &manifest))
}

#[tauri::command]
fn export_soul_json_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    path: String,
) -> Result<JsonExportReceipt, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    package::export_json(&conn, &soul_id, std::path::Path::new(&path))
}

#[tauri::command]
fn export_soul_markdown_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    path: String,
) -> Result<MarkdownExportReceipt, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    package::export_markdown(&conn, &soul_id, std::path::Path::new(&path))
}

#[tauri::command]
fn list_receipts_cmd(app: tauri::AppHandle) -> Result<Vec<ReceiptSummary>, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    package::list_local_receipts(&app_dir)
}

#[tauri::command]
fn delete_soul_cmd(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    soul_id: String,
) -> Result<DeletionReceipt, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    package::wipe_local_data(&conn, &app_dir, &soul_id)
}

#[tauri::command]
fn detect_clients_cmd(app: tauri::AppHandle) -> Result<Vec<integrations::ClientStatus>, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let binary = integrations::server_binary_path();
    Ok(integrations::detect_clients(&app_dir, &binary))
}

#[tauri::command]
fn connect_client_cmd(
    app: tauri::AppHandle,
    client: String,
) -> Result<integrations::ClientStatus, String> {
    let client: integrations::ClientId = client.parse().map_err(|e: String| e)?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let binary = integrations::server_binary_path();
    integrations::connect_client(&app_dir, &binary, client)
}

#[tauri::command]
fn disconnect_client_cmd(
    app: tauri::AppHandle,
    client: String,
) -> Result<integrations::ClientStatus, String> {
    let client: integrations::ClientId = client.parse().map_err(|e: String| e)?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    integrations::disconnect_client(&app_dir, client)
}

#[tauri::command]
fn rollback_client_cmd(
    app: tauri::AppHandle,
    client: String,
) -> Result<integrations::ClientStatus, String> {
    let client: integrations::ClientId = client.parse().map_err(|e: String| e)?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    integrations::rollback_client(&app_dir, client)
}

#[tauri::command]
fn register_bridge_cmd(app: tauri::AppHandle) -> Result<native_host::BridgeStatus, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    native_host::register_bridge(&app_dir)
}

#[tauri::command]
fn unregister_bridge_cmd(app: tauri::AppHandle) -> Result<native_host::BridgeStatus, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    native_host::unregister_bridge(&app_dir)
}

#[tauri::command]
fn bridge_status_cmd(app: tauri::AppHandle) -> Result<native_host::BridgeStatus, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(native_host::bridge_status(&app_dir))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // параметры = поля EvaluationRow, дублируются из eval::create_evaluation
fn create_evaluation_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
    scenario_id: String,
    scenario_text: String,
    domain: String,
    soul_answer: String,
    baseline_answer: String,
    baseline_profile: String,
    context_pack: String,
    context_entity_ids: Vec<String>,
) -> Result<eval::EvaluationRow, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    eval::create_evaluation(
        &conn,
        &soul_id,
        &scenario_id,
        &scenario_text,
        &domain,
        &soul_answer,
        &baseline_answer,
        &baseline_profile,
        &context_pack,
        &context_entity_ids,
    )
}

#[tauri::command]
fn submit_evaluation_choice_cmd(
    state: tauri::State<AppState>,
    evaluation_id: String,
    choice: String,
) -> Result<eval::EvaluationRow, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    eval::submit_choice(&conn, &evaluation_id, &choice)
}

#[tauri::command]
fn list_evaluations_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
) -> Result<Vec<eval::EvaluationRow>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    eval::list_evaluations(&conn, &soul_id)
}

#[tauri::command]
fn delete_evaluation_cmd(
    state: tauri::State<AppState>,
    evaluation_id: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    eval::delete_evaluation(&conn, &evaluation_id)
}

#[tauri::command]
fn create_policy_cmd(
    state: tauri::State<AppState>,
    rule_json: String,
) -> Result<policy::PolicyRow, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    policy::create_policy(&conn, &rule_json)
}

#[tauri::command]
fn list_policies_cmd(state: tauri::State<AppState>) -> Result<Vec<policy::PolicyRow>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    policy::list_policies(&conn)
}

#[tauri::command]
fn set_policy_enabled_cmd(
    state: tauri::State<AppState>,
    policy_id: String,
    enabled: bool,
) -> Result<policy::PolicyRow, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    policy::set_policy_enabled(&conn, &policy_id, enabled)
}

#[tauri::command]
fn delete_policy_cmd(state: tauri::State<AppState>, policy_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    policy::delete_policy(&conn, &policy_id)
}

#[tauri::command]
fn evaluate_action_cmd(
    state: tauri::State<AppState>,
    action_json: String,
) -> Result<policy::Decision, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let action: policy::SoulAction = serde_json::from_str(&action_json)
        .map_err(|e| format!("Action is not valid SoulAction JSON: {e}"))?;
    policy::evaluate(&conn, &action)
}

#[tauri::command]
fn gateway_propose_cmd(
    state: tauri::State<AppState>,
    action_json: String,
    ttl_seconds: Option<u64>,
) -> Result<gateway::GatewayProposal, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    gateway::propose_action(&conn, &action_json, ttl_seconds)
}

#[tauri::command]
fn gateway_execute_cmd(
    state: tauri::State<AppState>,
    capability_id: String,
    connector_id: String,
    account_id: String,
    environment: String,
    action_json: String,
) -> Result<gateway::GatewayExecuteResult, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    gateway::execute_capability(
        &conn,
        &capability_id,
        &connector_id,
        &account_id,
        &environment,
        &action_json,
    )
}

#[tauri::command]
fn list_gateway_receipts_cmd(
    state: tauri::State<AppState>,
) -> Result<Vec<gateway::GatewayReceipt>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    gateway::list_receipts(&conn)
}

#[tauri::command]
fn list_gateway_capabilities_cmd(
    state: tauri::State<AppState>,
) -> Result<Vec<gateway::CapabilityInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    gateway::list_capabilities(&conn)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            conn: Mutex::new(
                rusqlite::Connection::open_in_memory()
                    .expect("Failed to open placeholder database"),
            ),
        })
        .invoke_handler(tauri::generate_handler![
            health,
            init_app,
            create_soul_cmd,
            get_soul_cmd,
            add_entity_cmd,
            update_entity_cmd,
            list_entities_cmd,
            search_entities_cmd,
            get_calibration_cmd,
            save_calibration_cmd,
            activate_soul_cmd,
            confirm_preview_cmd,
            reset_preview_cmd,
            activate_preview_cmd,
            export_soul_cmd,
            inspect_soul_file_cmd,
            import_soul_file_cmd,
            export_soul_json_cmd,
            export_soul_markdown_cmd,
            delete_soul_cmd,
            list_receipts_cmd,
            detect_clients_cmd,
            connect_client_cmd,
            disconnect_client_cmd,
            rollback_client_cmd,
            register_bridge_cmd,
            unregister_bridge_cmd,
            bridge_status_cmd,
            create_evaluation_cmd,
            submit_evaluation_choice_cmd,
            list_evaluations_cmd,
            delete_evaluation_cmd,
            create_policy_cmd,
            list_policies_cmd,
            set_policy_enabled_cmd,
            delete_policy_cmd,
            evaluate_action_cmd,
            gateway_propose_cmd,
            gateway_execute_cmd,
            list_gateway_receipts_cmd,
            list_gateway_capabilities_cmd,
        ])
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
            let conn = init_db(&app_dir).expect("Failed to initialize database");
            let state = app.state::<AppState>();
            *state.conn.lock().unwrap() = conn;
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SOUL");
}
