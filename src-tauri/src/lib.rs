mod db;

use db::{init_db, create_soul, add_entity, list_entities, get_soul, list_souls};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    conn: Mutex<rusqlite::Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SoulInfo {
    soul_id: String,
    display_name: String,
    format_version: String,
    schema_version: String,
    created_at: String,
    head_event_hash: Option<String>,
    entity_count: i64,
    device_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityInfo {
    id: String,
    soul_id: String,
    entity_type: String,
    status: String,
    data: String,
    created_at: String,
    updated_at: String,
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
        Ok(SoulInfo {
            soul_id: s.soul_id.clone(),
            display_name: s.display_name.clone(),
            format_version: s.format_version.clone(),
            schema_version: s.schema_version.clone(),
            created_at: s.created_at.clone(),
            head_event_hash: s.head_event_hash.clone(),
            entity_count: s.entity_count,
            device_id: s.device_id.clone(),
        })
    } else {
        Err("No SOUL found. Create one first.".into())
    }
}

#[tauri::command]
fn create_soul_cmd(state: tauri::State<AppState>, display_name: String, device_id: String) -> Result<SoulInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let manifest = create_soul(&conn, &display_name, &device_id).map_err(|e| e.to_string())?;
    Ok(SoulInfo {
        soul_id: manifest.soul_id,
        display_name: manifest.display_name,
        format_version: manifest.format_version,
        schema_version: manifest.schema_version,
        created_at: manifest.created_at,
        head_event_hash: manifest.head_event_hash,
        entity_count: manifest.entity_count,
        device_id: manifest.device_id,
    })
}

#[tauri::command]
fn get_soul_cmd(state: tauri::State<AppState>, soul_id: String) -> Result<Option<SoulInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    match get_soul(&conn, &soul_id).map_err(|e| e.to_string())? {
        Some(s) => Ok(Some(SoulInfo {
            soul_id: s.soul_id,
            display_name: s.display_name,
            format_version: s.format_version,
            schema_version: s.schema_version,
            created_at: s.created_at,
            head_event_hash: s.head_event_hash,
            entity_count: s.entity_count,
            device_id: s.device_id,
        })),
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
    Ok(EntityInfo {
        id: row.id,
        soul_id: row.soul_id,
        entity_type: row.entity_type,
        status: row.status,
        data: row.data,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[tauri::command]
fn list_entities_cmd(state: tauri::State<AppState>, soul_id: String) -> Result<Vec<EntityInfo>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = list_entities(&conn, &soul_id).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| EntityInfo {
            id: r.id,
            soul_id: r.soul_id,
            entity_type: r.entity_type,
            status: r.status,
            data: r.data,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
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
                init_db(
                    &std::path::PathBuf::from("."),
                )
                .expect("Failed to initialize database"),
            ),
        })
        .invoke_handler(tauri::generate_handler![
            health,
            init_app,
            create_soul_cmd,
            get_soul_cmd,
            add_entity_cmd,
            list_entities_cmd,
        ])
        .setup(|app| {
            // Re-initialize DB with correct app data dir
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
            let conn = init_db(&app_dir).expect("Failed to initialize database");
            let state = app.state::<AppState>();
            *state.conn.lock().unwrap() = conn;
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SOUL");
}
