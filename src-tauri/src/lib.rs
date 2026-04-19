mod logging;
mod memory;
mod process_manager;
mod pty;
mod scaffold;
mod tab_store;

use logging::{LogBuffer, LogEntry};
use memory::{MemoryGraph, MemoryState};
use process_manager::{ManagedProcesses, ProcessStatus};
use pty::PtyState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

struct TabInfo {
    id: String,
    project_dir: String,
    memory_dir: PathBuf,
    pty: Option<PtyState>,
    pty_started_at: Option<String>,
    last_memory_save: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct TabStatus {
    pty_running: bool,
    pty_started_at: Option<String>,
    last_memory_save: Option<String>,
    context_percent: Option<f32>,
    tokens_used: Option<u64>,
    tokens_total: Option<u64>,
}

struct AppState {
    processes: ManagedProcesses,
    logs: LogBuffer,
    tabs: Mutex<HashMap<String, TabInfo>>,
    active_tab: Mutex<Option<String>>,
    app_data_dir: PathBuf,
    workspace_root: String, // where model-rewrite-proxy.js lives
    tab_store: Mutex<tab_store::TabStore>,
}

// ── Proxy commands (shared, unchanged) ───────────────────────────

#[tauri::command]
fn start_copilot_proxy(state: State<AppState>, app: AppHandle) -> Result<u16, String> {
    state.processes.cleanup_orphans(&app, &state.logs);
    state.processes.start_copilot_proxy(&app, &state.logs)
}

#[tauri::command]
fn start_model_rewriter(state: State<AppState>, app: AppHandle, upstream_port: u16) -> Result<u16, String> {
    state.processes.start_model_rewriter(&app, &state.logs, &state.workspace_root, upstream_port)
}

#[tauri::command]
fn stop_services(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    logging::emit_log(&app, &state.logs, "app", "info", "Stopping all services");
    state.processes.stop_all();
    Ok(())
}

#[tauri::command]
fn get_process_statuses(state: State<AppState>) -> Vec<ProcessStatus> {
    state.processes.get_statuses()
}

// ── Tab management commands ──────────────────────────────────────

#[tauri::command]
fn create_tab(state: State<AppState>, app: AppHandle, project_path: String) -> Result<String, String> {
    // Scaffold project if needed
    scaffold::scaffold_project(&project_path)?;

    let tab_id = uuid::Uuid::new_v4().to_string();
    let memory_dir = PathBuf::from(&project_path).join(".claude-memory");

    eprintln!("[workspace] Creating tab {} for {:?}", tab_id, project_path);
    logging::emit_log(&app, &state.logs, "app", "info", &format!("Creating tab for {}", project_path));

    // Spawn PTY for this tab
    let rewriter_port = state.processes.get_rewriter_port();
    let pty_state = pty::spawn_pty(&app, &tab_id, &project_path, rewriter_port, memory_dir.exists())?;

    // Start memory watcher for this tab's memory dir
    if memory_dir.exists() {
        let watcher_tab_id = tab_id.clone();
        memory::start_watcher_for_tab(app.clone(), memory_dir.clone(), watcher_tab_id);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let tab_info = TabInfo {
        id: tab_id.clone(),
        project_dir: project_path.clone(),
        memory_dir,
        pty: Some(pty_state),
        pty_started_at: Some(now.clone()),
        last_memory_save: None,
    };

    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.insert(tab_id.clone(), tab_info);
    drop(tabs);

    // Set as active tab
    *state.active_tab.lock().map_err(|e| e.to_string())? = Some(tab_id.clone());

    // Persist
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::add_tab(&mut store, tab_id.clone(), project_path, "project");
    tab_store::update_tab_stats(&mut store, &tab_id, Some(now.clone()), None);
    tab_store::save_tabs(&state.app_data_dir, &store);

    Ok(tab_id)
}

#[tauri::command]
fn create_scratch_tab(state: State<AppState>, app: AppHandle) -> Result<String, String> {
    let tab_id = uuid::Uuid::new_v4().to_string();

    // Use a temp directory for scratch tabs
    let temp_dir = std::env::temp_dir().join(format!("proximus-scratch-{}", &tab_id[..8]));
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let project_path = temp_dir.to_string_lossy().to_string();

    eprintln!("[workspace] Creating scratch tab {} at {:?}", tab_id, project_path);
    logging::emit_log(&app, &state.logs, "app", "info", &format!("Creating scratch tab"));

    // Spawn PTY with no memory
    let rewriter_port = state.processes.get_rewriter_port();
    let pty_state = pty::spawn_pty(&app, &tab_id, &project_path, rewriter_port, false)?;

    let tab_info = TabInfo {
        id: tab_id.clone(),
        project_dir: project_path.clone(),
        memory_dir: temp_dir.join(".claude-memory"), // won't exist
        pty: Some(pty_state),
        pty_started_at: Some(chrono::Utc::now().to_rfc3339()),
        last_memory_save: None,
    };

    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.insert(tab_id.clone(), tab_info);
    drop(tabs);

    *state.active_tab.lock().map_err(|e| e.to_string())? = Some(tab_id.clone());

    // Persist with a "Chat" name
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    let ts = format!("{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs());
    // Count existing chat tabs for naming
    let chat_count = store.tabs.iter().filter(|t| t.tab_type == "chat").count();
    let chat_name = if chat_count == 0 {
        "Chat".to_string()
    } else {
        format!("Chat {}", chat_count + 1)
    };

    let now = chrono::Utc::now().to_rfc3339();
    store.tabs.push(tab_store::TabState {
        id: tab_id.clone(),
        project_path,
        project_name: chat_name,
        status: "active".into(),
        tab_type: "chat".into(),
        last_opened: ts.clone(),
        created_at: ts,
        pty_started_at: Some(now),
        last_memory_save: None,
    });
    store.active_tab_id = Some(tab_id.clone());
    tab_store::save_tabs(&state.app_data_dir, &store);

    Ok(tab_id)
}

#[tauri::command]
fn close_tab(state: State<AppState>) -> Result<(), String> {
    let tab_id = {
        let active = state.active_tab.lock().map_err(|e| e.to_string())?;
        active.clone().ok_or("No active tab")?
    };

    // Remove PTY (dropping it kills the process)
    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.remove(&tab_id);
    drop(tabs);

    // Update persistence
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::close_tab(&mut store, &tab_id);
    tab_store::save_tabs(&state.app_data_dir, &store);

    // Switch active to next available
    *state.active_tab.lock().map_err(|e| e.to_string())? = store.active_tab_id.clone();

    Ok(())
}

#[tauri::command]
fn close_tab_by_id(state: State<AppState>) -> Result<(), String> {
    close_tab(state)
}

#[tauri::command]
fn switch_tab(state: State<AppState>, tab_id: String) -> Result<(), String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    if !tabs.contains_key(&tab_id) {
        return Err(format!("Tab {} not found", tab_id));
    }
    drop(tabs);

    *state.active_tab.lock().map_err(|e| e.to_string())? = Some(tab_id.clone());

    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    store.active_tab_id = Some(tab_id);
    tab_store::save_tabs(&state.app_data_dir, &store);

    Ok(())
}

#[tauri::command]
fn get_tabs(state: State<AppState>) -> Result<Vec<tab_store::TabState>, String> {
    let store = state.tab_store.lock().map_err(|e| e.to_string())?;
    Ok(store.tabs.clone())
}

#[tauri::command]
fn get_active_tab_id(state: State<AppState>) -> Result<Option<String>, String> {
    let active = state.active_tab.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}

#[tauri::command]
fn reopen_tab(state: State<AppState>, app: AppHandle, tab_id: String) -> Result<(), String> {
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    let tab_state = store
        .tabs
        .iter()
        .find(|t| t.id == tab_id)
        .cloned()
        .ok_or("Tab not found in store")?;

    let reopen_started_at = chrono::Utc::now().to_rfc3339();
    tab_store::reopen_tab(&mut store, &tab_id);
    tab_store::update_tab_stats(&mut store, &tab_id, Some(reopen_started_at.clone()), None);
    tab_store::save_tabs(&state.app_data_dir, &store);
    drop(store);

    // Re-spawn PTY
    let memory_dir = PathBuf::from(&tab_state.project_path).join(".claude-memory");
    let rewriter_port = state.processes.get_rewriter_port();
    let pty_state = pty::spawn_pty(&app, &tab_id, &tab_state.project_path, rewriter_port, memory_dir.exists())?;

    if memory_dir.exists() {
        memory::start_watcher_for_tab(app.clone(), memory_dir.clone(), tab_id.clone());
    }

    let tab_info = TabInfo {
        id: tab_id.clone(),
        project_dir: tab_state.project_path,
        memory_dir,
        pty: Some(pty_state),
        pty_started_at: Some(reopen_started_at),
        last_memory_save: tab_state.last_memory_save,
    };

    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.insert(tab_id.clone(), tab_info);
    drop(tabs);

    *state.active_tab.lock().map_err(|e| e.to_string())? = Some(tab_id);

    Ok(())
}

#[tauri::command]
fn remove_tab(state: State<AppState>, tab_id: String) -> Result<(), String> {
    // Remove from running tabs if present
    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.remove(&tab_id);
    drop(tabs);

    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::remove_tab(&mut store, &tab_id);
    tab_store::save_tabs(&state.app_data_dir, &store);

    if state.active_tab.lock().map_err(|e| e.to_string())?.as_deref() == Some(&tab_id) {
        *state.active_tab.lock().map_err(|e| e.to_string())? = store.active_tab_id.clone();
    }

    Ok(())
}

// ── Tab-scoped PTY commands ──────────────────────────────────────

#[tauri::command]
fn spawn_tab_pty(state: State<AppState>, app: AppHandle, tab_id: String) -> Result<(), String> {
    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get_mut(&tab_id).ok_or("Tab not found")?;
    if tab.pty.is_some() {
        return Ok(()); // Already has a PTY
    }
    let rewriter_port = state.processes.get_rewriter_port();
    let has_memory = tab.memory_dir.exists();
    let pty_state = pty::spawn_pty(&app, &tab_id, &tab.project_dir, rewriter_port, has_memory)?;
    tab.pty = Some(pty_state);
    let now = chrono::Utc::now().to_rfc3339();
    tab.pty_started_at = Some(now.clone());
    drop(tabs);

    // Persist pty_started_at
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::update_tab_stats(&mut store, &tab_id, Some(now), None);
    tab_store::save_tabs(&state.app_data_dir, &store);
    drop(store);

    // Start memory watcher
    {
        let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
        let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
        if tab.memory_dir.exists() {
            memory::start_watcher_for_tab(app.clone(), tab.memory_dir.clone(), tab_id);
        }
    }
    Ok(())
}

#[tauri::command]
fn write_pty(state: State<AppState>, tab_id: String, data: String) -> Result<(), String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    match tab.pty.as_ref() {
        Some(pty_state) => pty::write_to_pty(pty_state, &data),
        None => Err("PTY not initialized for this tab".into()),
    }
}

#[tauri::command]
fn resize_pty_cmd(state: State<AppState>, tab_id: String, rows: u16, cols: u16) -> Result<(), String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    match tab.pty.as_ref() {
        Some(pty_state) => pty::resize_pty(pty_state, rows, cols),
        None => Err("PTY not initialized for this tab".into()),
    }
}

// ── Tab status command ───────────────────────────────────────────

#[tauri::command]
fn get_tab_status(state: State<AppState>, tab_id: String) -> Result<TabStatus, String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    Ok(TabStatus {
        pty_running: tab.pty.is_some(),
        pty_started_at: tab.pty_started_at.clone(),
        last_memory_save: tab.last_memory_save.clone(),
        context_percent: None,
        tokens_used: None,
        tokens_total: None,
    })
}

/// Get context usage by reading Claude Code's session JSONL files
#[tauri::command]
fn get_context_usage(tab_id: String, state: State<AppState>) -> Result<serde_json::Value, String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    let project_dir = tab.project_dir.clone();
    drop(tabs);

    let empty = serde_json::json!({
        "used_percentage": 0,
        "context_window_size": 200000,
        "total_input_tokens": 0,
        "total_output_tokens": 0,
        "cost_usd": 0,
        "model": ""
    });

    // Scan all project-*.json files in ~/.claude/proximus-stats/
    // and find the one whose "cwd" matches our project_dir
    let home = dirs::home_dir().ok_or("No home dir")?;
    let stats_dir = home.join(".claude").join("proximus-stats");

    if !stats_dir.exists() {
        return Ok(empty);
    }

    // Normalize project_dir for comparison (forward slashes, lowercase on Windows)
    let norm_project = project_dir.replace('\\', "/").to_lowercase();

    let mut best: Option<(std::time::SystemTime, serde_json::Value)> = None;

    if let Ok(entries) = std::fs::read_dir(&stats_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("project-")).unwrap_or(false) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(cwd) = data.get("cwd").and_then(|v| v.as_str()) {
                        let norm_cwd = cwd.replace('\\', "/").to_lowercase();
                        if norm_cwd == norm_project {
                            let mtime = path.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                            if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                                best = Some((mtime, data));
                            }
                        }
                    }
                }
            }
        }
    }

    match best {
        Some((_, data)) => Ok(data),
        None => Ok(empty),
    }
}

#[tauri::command]
fn update_memory_save_time(state: State<AppState>, tab_id: String, timestamp: String) -> Result<(), String> {
    // Update runtime
    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    if let Some(tab) = tabs.get_mut(&tab_id) {
        tab.last_memory_save = Some(timestamp.clone());
    }
    drop(tabs);

    // Persist
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::update_tab_stats(&mut store, &tab_id, None, Some(timestamp));
    tab_store::save_tabs(&state.app_data_dir, &store);
    Ok(())
}

// ── Tab-scoped memory commands ───────────────────────────────────

#[tauri::command]
fn get_memory_graph(state: State<AppState>, tab_id: String) -> Result<MemoryGraph, String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    memory::parse_graph(&tab.memory_dir)
}

#[tauri::command]
fn get_memory_state(state: State<AppState>, tab_id: String) -> Result<MemoryState, String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    memory::parse_state(&tab.memory_dir)
}

#[tauri::command]
fn get_node_content(state: State<AppState>, tab_id: String, node_id: String) -> Result<String, String> {
    let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    let tab = tabs.get(&tab_id).ok_or("Tab not found")?;
    memory::read_node_file(&tab.memory_dir, &node_id)
}

// ── File explorer commands ───────────────────────────────────────

#[tauri::command]
async fn open_project_folder(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

// ── Logging commands ─────────────────────────────────────────────

#[tauri::command]
fn get_log_history(state: State<AppState>) -> Vec<LogEntry> {
    state.logs.get_all()
}

// ── App setup ────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));

            eprintln!("[workspace] app_data_dir = {:?}", app_data_dir);

            // Load persisted tab store
            let store = tab_store::load_tabs(&app_data_dir);
            eprintln!("[workspace] Loaded {} persisted tabs", store.tabs.len());

            // Populate runtime tabs from persisted active tabs (without PTY)
            let mut runtime_tabs = HashMap::new();
            for tab in &store.tabs {
                if tab.status == "active" {
                    let memory_dir = PathBuf::from(&tab.project_path).join(".claude-memory");
                    runtime_tabs.insert(tab.id.clone(), TabInfo {
                        id: tab.id.clone(),
                        project_dir: tab.project_path.clone(),
                        memory_dir,
                        pty: None,
                        pty_started_at: tab.pty_started_at.clone(),
                        last_memory_save: tab.last_memory_save.clone(),
                    });
                }
            }
            eprintln!("[workspace] Restored {} active tabs (without PTY)", runtime_tabs.len());

            // Find workspace root (where model-rewrite-proxy.js lives)
            // Walk up from the executable's directory
            let workspace_root = {
                let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let mut found = dir.clone();
                for _ in 0..5 {
                    if dir.join("model-rewrite-proxy.js").exists() {
                        found = dir.clone();
                        break;
                    }
                    if !dir.pop() { break; }
                }
                found.to_string_lossy().to_string()
            };
            eprintln!("[workspace] workspace_root = {:?}", workspace_root);

            app.manage(AppState {
                processes: ManagedProcesses::new(),
                logs: LogBuffer::new(),
                tabs: Mutex::new(runtime_tabs),
                active_tab: Mutex::new(store.active_tab_id.clone()),
                app_data_dir,
                workspace_root,
                tab_store: Mutex::new(store),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Proxy (shared)
            start_copilot_proxy,
            start_model_rewriter,
            stop_services,
            get_process_statuses,
            // Tab management
            create_tab,
            create_scratch_tab,
            close_tab,
            close_tab_by_id,
            switch_tab,
            get_tabs,
            get_active_tab_id,
            reopen_tab,
            remove_tab,
            // Tab-scoped PTY
            spawn_tab_pty,
            write_pty,
            resize_pty_cmd,
            // Tab status
            get_tab_status,
            update_memory_save_time,
            get_context_usage,
            // Tab-scoped memory
            get_memory_graph,
            get_memory_state,
            get_node_content,
            // Logging
            get_log_history,
            // File explorer
            open_project_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                eprintln!("[workspace] App exit requested — cleaning up processes");
                if let Some(state) = app.try_state::<AppState>() {
                    logging::emit_log(
                        app,
                        &state.logs,
                        "app",
                        "info",
                        "Shutting down — killing all child processes",
                    );
                    // Kill proxy process trees
                    state.processes.stop_all();
                    // Drop all PTYs (kills shells)
                    if let Ok(mut tabs) = state.tabs.lock() {
                        for (_, tab) in tabs.iter_mut() {
                            tab.pty = None;
                        }
                        tabs.clear();
                    }
                }
            }
        });
}
