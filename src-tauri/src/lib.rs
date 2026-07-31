mod db;

use db::{
    init_db, create_soul, add_entity, list_entities, get_soul, list_souls,
    get_calibration, save_calibration, activate_soul, is_soul_activated,
    update_entity,
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

fn soul_to_info(
    conn: &rusqlite::Connection,
    s: &db::SoulManifest,
) -> SoulInfo {
    let activated = is_soul_activated(conn, &s.soul_id).unwrap_or(false);
    let cstep = get_calibration(conn, &s.soul_id)
        .map(|(s, _)| s)
        .unwrap_or(0);
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
    data: String,
    device_id: String,
) -> Result<EntityInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let row = update_entity(&conn, &entity_id, &status, &data, &device_id)
        .map_err(|e| e.to_string())?;
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
fn activate_soul_cmd(
    state: tauri::State<AppState>,
    soul_id: String,
) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    activate_soul(&conn, &soul_id).map_err(|e| e.to_string())?;
    let s = get_soul(&conn, &soul_id)
        .map_err(|e| e.to_string())?
        .ok_or("SOUL not found".to_string())?;
    Ok(soul_to_info(&conn, &s))
}

fn list_souls_internal(state: &AppState) -> Result<Vec<db::SoulManifest>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    list_souls(&conn).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            conn: Mutex::new(
                init_db(&std::path::PathBuf::from("."))
                    .expect("Failed to initialize database"),
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
            get_calibration_cmd,
            save_calibration_cmd,
            activate_soul_cmd,
        ])
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
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
