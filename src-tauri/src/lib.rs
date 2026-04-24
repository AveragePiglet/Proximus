mod app_settings;
mod file_sync;
mod logging;
mod memory;
mod model_rewriter;
mod process_manager;
mod pty;
mod scaffold;
mod tab_store;

use app_settings::AppSettings;
use logging::{LogBuffer, LogEntry};
use memory::{MemoryGraph, MemoryState};
use process_manager::{ManagedProcesses, ProcessStatus};
use pty::PtyState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

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

#[derive(Clone, serde::Serialize)]
struct CreateTabResult {
    tab_id: String,
    memory_migrated: bool,
    dirs_added: Vec<String>,
}

struct AppState {
    processes: ManagedProcesses,
    logs: LogBuffer,
    tabs: Mutex<HashMap<String, TabInfo>>,
    active_tab: Mutex<Option<String>>,
    app_data_dir: PathBuf,
    tab_store: Mutex<tab_store::TabStore>,
    settings: Mutex<AppSettings>,
    /// PID of a running `npx copilot-api auth` process, so it can be cancelled
    pending_auth_pid: Mutex<Option<u32>>,
}

// ── Proxy commands (shared, unchanged) ───────────────────────────

#[tauri::command]
fn start_copilot_proxy(state: State<AppState>, app: AppHandle) -> Result<u16, String> {
    state.processes.cleanup_orphans(&app, &state.logs);
    state.processes.start_copilot_proxy(&app, &state.logs)
}

#[tauri::command]
async fn start_model_rewriter(state: State<'_, AppState>, app: AppHandle, upstream_port: u16) -> Result<u16, String> {
    state.processes.start_model_rewriter(&app, &state.logs, upstream_port).await
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
fn detect_project_memory(project_path: String) -> Result<Vec<String>, String> {
    scaffold::detect_existing_memory(&project_path)
}

#[tauri::command]
fn scaffold_project_cmd(project_path: String) -> Result<bool, String> {
    scaffold::scaffold_project(&project_path)
}

#[tauri::command]
fn sync_memory_structure(project_path: String) -> Result<Vec<String>, String> {
    scaffold::ensure_memory_structure(&project_path)
}

#[tauri::command]
fn update_claude_md_references(project_path: String) -> Result<bool, String> {
    scaffold::update_claude_md_references(&project_path)
}

#[tauri::command]
fn get_migration_file_contents(project_path: String, files: Vec<String>) -> Result<Vec<(String, String)>, String> {
    let root = std::path::Path::new(&project_path);
    let mut results = Vec::new();
    for rel in &files {
        let p = root.join(rel);
        if p.is_file() {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| format!("Failed to read {}: {}", rel, e))?;
            results.push((rel.clone(), content));
        } else if p.is_dir() {
            // For directories, list their files
            let mut entries = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&p) {
                for entry in rd.flatten() {
                    if entry.path().is_file() {
                        if let Ok(c) = std::fs::read_to_string(entry.path()) {
                            let name = format!("{}/{}", rel, entry.file_name().to_string_lossy());
                            entries.push((name, c));
                        }
                    }
                }
            }
            results.extend(entries);
        }
    }
    Ok(results)
}

#[tauri::command]
fn create_tab(state: State<AppState>, app: AppHandle, project_path: String) -> Result<CreateTabResult, String> {
    // Auto-migrate legacy .claude-memory → .node-memory if present
    let migrated = scaffold::migrate_legacy_memory(&project_path)?;
    if migrated {
        logging::emit_log(&app, &state.logs, "app", "info",
            &format!("Migrated .claude-memory → .node-memory for {}", project_path));
    }

    // Only scaffold if no existing memory detected (migration handled separately by frontend)
    let memory_dir = PathBuf::from(&project_path).join(".node-memory");
    if !memory_dir.exists() {
        let existing = scaffold::detect_existing_memory(&project_path)?;
        if existing.is_empty() {
            // Fresh project — scaffold immediately
            scaffold::scaffold_project(&project_path)?;
        }
        // else: has existing memory — frontend will handle migration dialog
    }

    // Sync structure: add any dirs/files missing from an already-existing .node-memory/
    let dirs_added = scaffold::ensure_memory_structure(&project_path)?;
    if !dirs_added.is_empty() {
        logging::emit_log(&app, &state.logs, "app", "info",
            &format!("Memory structure synced — added: {}", dirs_added.join(", ")));
    }

    let tab_id = uuid::Uuid::new_v4().to_string();
    let memory_dir = PathBuf::from(&project_path).join(".node-memory");

    eprintln!("[workspace] Creating tab {} for {:?}", tab_id, project_path);
    logging::emit_log(&app, &state.logs, "app", "info", &format!("Creating tab for {}", project_path));

    // Spawn PTY for this tab — use project primary model from settings
    let rewriter_port = state.processes.get_rewriter_port();
    let (model, fallback_model, dangerous_mode, cli_mode, copilot_model) = {
        let s = state.settings.lock().map_err(|e| e.to_string())?;
        (Some(s.project_primary_model.clone()), Some(s.project_secondary_model.clone()), s.dangerously_skip_permissions, s.cli_mode.clone(), s.copilot_model.clone())
    };
    let pty_state = pty::spawn_pty(&app, &tab_id, &project_path, rewriter_port, memory_dir.exists(), model, fallback_model, dangerous_mode, &cli_mode, Some(copilot_model))?;

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

    Ok(CreateTabResult { tab_id, memory_migrated: migrated, dirs_added })
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

    // Spawn PTY with no memory — use chat model from settings
    let rewriter_port = state.processes.get_rewriter_port();
    let (model, dangerous_mode, cli_mode, copilot_model) = {
        let s = state.settings.lock().map_err(|e| e.to_string())?;
        (Some(s.chat_model.clone()), s.dangerously_skip_permissions, s.cli_mode.clone(), s.copilot_model.clone())
    };
    let pty_state = pty::spawn_pty(&app, &tab_id, &project_path, rewriter_port, false, model, None, dangerous_mode, &cli_mode, Some(copilot_model))?;

    let tab_info = TabInfo {
        id: tab_id.clone(),
        project_dir: project_path.clone(),
        memory_dir: temp_dir.join(".node-memory"), // won't exist
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
fn close_all_tabs(state: State<AppState>) -> Result<(), String> {
    // Drop all PTY handles (kills the shell processes)
    let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
    tabs.clear();
    drop(tabs);

    // Clear active tab pointer
    *state.active_tab.lock().map_err(|e| e.to_string())? = None;

    // Persist: mark all as closed
    let mut store = state.tab_store.lock().map_err(|e| e.to_string())?;
    tab_store::close_all(&mut store);
    tab_store::save_tabs(&state.app_data_dir, &store);

    Ok(())
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

    // Re-spawn PTY — use project primary model from settings
    let memory_dir = PathBuf::from(&tab_state.project_path).join(".node-memory");
    let rewriter_port = state.processes.get_rewriter_port();
    let (model, fallback_model, dangerous_mode, cli_mode, copilot_model) = {
        let s = state.settings.lock().map_err(|e| e.to_string())?;
        (Some(s.project_primary_model.clone()), Some(s.project_secondary_model.clone()), s.dangerously_skip_permissions, s.cli_mode.clone(), s.copilot_model.clone())
    };

    // Auto-seed Copilot/Claude files on first open in each mode
    {
        let project_dir = &tab_state.project_path;
        let copilot_instructions = std::path::Path::new(project_dir)
            .join(".github")
            .join("copilot-instructions.md");
        let claude_md = std::path::Path::new(project_dir).join("CLAUDE.md");
        eprintln!("[sync] check: cli_mode={} project={} claude_md_exists={} copilot_instructions_exists={}",
            cli_mode, project_dir, claude_md.exists(), copilot_instructions.exists());
        let should_sync = if cli_mode == "copilot" {
            !copilot_instructions.exists() && claude_md.exists()
        } else {
            !claude_md.exists() && copilot_instructions.exists()
        };
        if should_sync {
            let (from_mode, to_mode) = if cli_mode == "copilot" { ("claude", "copilot") } else { ("copilot", "claude") };
            eprintln!("[sync] auto-seeding {} → {} for {}", from_mode, to_mode, project_dir);
            if let Err(e) = file_sync::sync_cli_files(project_dir, from_mode, to_mode) {
                eprintln!("[sync] auto-seed failed (non-fatal): {}", e);
            }
        }
    }

    let pty_state = pty::spawn_pty(&app, &tab_id, &tab_state.project_path, rewriter_port, memory_dir.exists(), model, fallback_model, dangerous_mode, &cli_mode, Some(copilot_model))?;

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
    let (model, fallback_model, dangerous_mode, cli_mode, copilot_model) = {
        let s = state.settings.lock().map_err(|e| e.to_string())?;
        (Some(s.project_primary_model.clone()), Some(s.project_secondary_model.clone()), s.dangerously_skip_permissions, s.cli_mode.clone(), s.copilot_model.clone())
    };
    // Auto-sync on first open: if switching into copilot mode and .github/copilot-instructions.md
    // is missing but CLAUDE.md exists, seed the Copilot files from Claude's layout.
    // Likewise, if switching into claude mode and CLAUDE.md is missing but copilot-instructions
    // exists, seed from Copilot. This avoids Copilot asking to /init on every new project.
    {
        let project_dir = &tab.project_dir;
        let copilot_instructions = std::path::Path::new(project_dir)
            .join(".github")
            .join("copilot-instructions.md");
        let claude_md = std::path::Path::new(project_dir).join("CLAUDE.md");

        eprintln!("[sync] check: cli_mode={} project={} claude_md_exists={} copilot_instructions_exists={}",
            cli_mode, project_dir, claude_md.exists(), copilot_instructions.exists());

        let should_sync = if cli_mode == "copilot" {
            !copilot_instructions.exists() && claude_md.exists()
        } else {
            !claude_md.exists() && copilot_instructions.exists()
        };

        if should_sync {
            let (from_mode, to_mode) = if cli_mode == "copilot" {
                ("claude", "copilot")
            } else {
                ("copilot", "claude")
            };
            eprintln!("[sync] auto-seeding {} → {} for {}", from_mode, to_mode, project_dir);
            if let Err(e) = file_sync::sync_cli_files(project_dir, from_mode, to_mode) {
                eprintln!("[sync] auto-seed failed (non-fatal): {}", e);
            }
        }
    }

    let pty_state = pty::spawn_pty(&app, &tab_id, &tab.project_dir, rewriter_port, has_memory, model, fallback_model, dangerous_mode, &cli_mode, Some(copilot_model))?;
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

#[tauri::command]
fn get_app_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
fn save_app_settings(state: State<AppState>, settings: AppSettings) -> Result<(), String> {
    let mut current = state.settings.lock().map_err(|e| e.to_string())?;
    *current = settings.clone();
    app_settings::save_settings(&state.app_data_dir, &settings);
    Ok(())
}

#[tauri::command]
fn get_available_models() -> Vec<app_settings::ModelEntry> {
    app_settings::get_available_models()
}

#[tauri::command]
fn get_copilot_models() -> Vec<app_settings::ModelEntry> {
    app_settings::get_copilot_models()
}

#[tauri::command]
fn scan_copilot_models() -> Vec<app_settings::ModelEntry> {
    app_settings::scan_copilot_models()
}

/// All candidate paths where copilot-api may store the GitHub token.
/// Different versions of copilot-api use different locations.
fn copilot_token_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();

    // Variant 1: ~/.local/share/copilot-api/github_token  (XDG data dir — used by current versions on all platforms)
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("share").join("copilot-api").join("github_token"));
    }

    // Variant 2: %LOCALAPPDATA%/copilot-api/github_token  (older Windows versions)
    if let Some(local) = dirs::data_local_dir() {
        candidates.push(local.join("copilot-api").join("github_token"));
    }

    // Variant 3: ~/.copilot-api/github_token  (some older versions)
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".copilot-api").join("github_token"));
    }

    // Variant 4: %APPDATA%/copilot-api/github_token  (roaming profile)
    if let Some(config) = dirs::config_dir() {
        candidates.push(config.join("copilot-api").join("github_token"));
    }

    candidates
}

#[tauri::command]
fn get_copilot_auth_status() -> bool {
    let found = copilot_token_candidates().iter().any(|p| {
        if !p.exists() { return false; }
        // A valid token file must have meaningful content — not empty or whitespace only
        match std::fs::read_to_string(p) {
            Ok(content) => content.trim().len() > 10,
            Err(_) => false,
        }
    });
    eprintln!(
        "[copilot-auth] token check: {} (searched: {:?})",
        found,
        copilot_token_candidates()
    );
    found
}

#[tauri::command]
fn sign_out_copilot(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    let mut removed = false;
    for path in copilot_token_candidates() {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove token at {:?}: {}", path, e))?;
            eprintln!("[copilot-auth] Removed token at {:?}", path);
            removed = true;
        }
    }
    if !removed {
        eprintln!("[copilot-auth] sign_out: no token file found");
    }

    // Kill the proxy and rewriter so they don't keep serving with the cached token
    logging::emit_log(&app, &state.logs, "app", "info", "Signed out — stopping Copilot proxy");
    state.processes.stop_all();

    Ok(())
}

#[tauri::command]
fn start_copilot_auth(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Kill any already-running auth process first
    cancel_copilot_auth(state.clone())?;

    let mut cmd = {
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.args(["/c", "npx", "copilot-api", "auth"]);
            c.creation_flags(CREATE_NO_WINDOW);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = Command::new("npx");
            c.args(["copilot-api", "auth"]);
            c
        }
    };

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start copilot auth: {}", e))?;

    // Store PID so cancel_copilot_auth can kill it later
    *state.pending_auth_pid.lock().map_err(|e| e.to_string())? = Some(child.id());

    let stdout = child.stdout.take().ok_or("No stdout")?;
    let stderr = child.stderr.take().ok_or("No stderr")?;

    // Read stdout
    let app_stdout = app.clone();
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("[copilot-auth:stdout] {}", line);
                let _ = app_stdout.emit("copilot-auth-output", line);
            }
        }
    });

    // Read stderr (device codes often appear here)
    let app_stderr = app.clone();
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("[copilot-auth:stderr] {}", line);
                let _ = app_stderr.emit("copilot-auth-output", line);
            }
        }
    });

    // Wait for the child process in a separate thread
    let app_wait = app.clone();
    std::thread::spawn(move || {
        let success = child.wait().map(|s| s.success()).unwrap_or(false);
        let _ = app_wait.emit("copilot-auth-done", success);
    });

    Ok(())
}

#[tauri::command]
fn cancel_copilot_auth(state: State<AppState>) -> Result<(), String> {
    if let Ok(mut pid_guard) = state.pending_auth_pid.lock() {
        if let Some(pid) = pid_guard.take() {
            eprintln!("[copilot-auth] Cancelling auth process PID {}", pid);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                let _ = std::process::Command::new("taskkill")
                    .args(["/T", "/F", "/PID", &pid.to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output();
            }
        }
    }
    Ok(())
}

// ── File sync command ────────────────────────────────────────────

#[tauri::command]
fn sync_cli_files(
    project_path: String,
    from_mode: String,
    to_mode: String,
) -> Result<file_sync::SyncResult, String> {
    eprintln!("[sync] {} → {} (project={})", from_mode, to_mode, project_path);
    file_sync::sync_cli_files(&project_path, &from_mode, &to_mode)
}

// ── CLI mode switch ──────────────────────────────────────────────

#[tauri::command]
async fn apply_cli_mode(
    state: State<'_, AppState>,
    app: AppHandle,
    mode: String,
    sync_files: bool,
) -> Result<(), String> {
    logging::emit_log(
        &app,
        &state.logs,
        "app",
        "info",
        &format!("Applying CLI mode switch → {}", mode),
    );

    match mode.as_str() {
        "copilot" => {
            // stop_all and cleanup_orphans are blocking — run off the async thread
            let processes = state.processes.clone();
            let app2 = app.clone();
            let logs = state.logs.clone();
            tauri::async_runtime::spawn_blocking(move || {
                processes.stop_all();
                processes.cleanup_orphans(&app2, &logs);
            }).await.map_err(|e| e.to_string())?;
            // Notify frontend badges that proxies are down
            let _ = app.emit("process-status", serde_json::json!({"name": "copilot-proxy", "running": false, "port": null}));
            let _ = app.emit("process-status", serde_json::json!({"name": "model-rewriter", "running": false, "port": null}));
            logging::emit_log(&app, &state.logs, "app", "info", "Proxies stopped for Copilot CLI mode");
        }
        "claude" => {
            // cleanup_orphans is blocking — run off the async thread first
            let processes = state.processes.clone();
            let app2 = app.clone();
            let logs = state.logs.clone();
            tauri::async_runtime::spawn_blocking(move || {
                processes.cleanup_orphans(&app2, &logs);
            }).await.map_err(|e| e.to_string())?;

            let copilot_port = state.processes.start_copilot_proxy(&app, &state.logs)?;
            state.processes.start_model_rewriter(&app, &state.logs, copilot_port).await?;
            logging::emit_log(&app, &state.logs, "app", "info", "Proxies restarted for Claude CLI mode");
        }
        _ => return Err(format!("Unknown CLI mode: {}", mode)),
    }

    let _ = sync_files; // handled by frontend via sync_cli_files (Step 7)

    let _ = app.emit("cli-mode-applied", serde_json::json!({
        "mode": mode,
        "sync_files": sync_files,
    }));

    Ok(())
}

// ── App setup ────────────────────────────────────────────────────

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct DependencyStatus {
    claude_installed: bool,
    copilot_api_installed: bool,
    copilot_cli_installed: bool,
}

#[tauri::command]
fn check_dependencies() -> DependencyStatus {
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let claude_installed = {
        let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.args(["/c", "claude", "--version"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(not(windows))]
        {
            cmd.args(["-c", "claude --version"]);
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    let copilot_api_installed = {
        // Use `npm list -g` instead of `npx --version` — npx is slow (downloads
        // the package if not cached) and unreliable on Windows.  `npm list -g`
        // returns exit-code 0 iff the package is globally installed, fast & offline.
        let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.args(["/c", "npm", "list", "-g", "copilot-api", "--depth=0"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(not(windows))]
        {
            cmd.args(["-c", "npm list -g copilot-api --depth=0"]);
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    let copilot_cli_installed = {
        let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.args(["/c", "copilot", "--version"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(not(windows))]
        {
            cmd.args(["-c", "copilot --version"]);
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    eprintln!(
        "[deps] claude_installed={}, copilot_api_installed={}, copilot_cli_installed={}",
        claude_installed, copilot_api_installed, copilot_cli_installed
    );

    DependencyStatus {
        claude_installed,
        copilot_api_installed,
        copilot_cli_installed,
    }
}

#[tauri::command]
async fn install_dependencies(install_claude: bool, install_copilot_api: bool, install_copilot_cli: bool) -> Result<String, String> {
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut results = Vec::new();

    if install_claude {
        let output = {
            let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.args(["/c", "npm", "install", "-g", "@anthropic-ai/claude-code"]);
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            #[cfg(not(windows))]
            {
                cmd.args(["-c", "npm install -g @anthropic-ai/claude-code"]);
            }
            cmd.output().map_err(|e| format!("Failed to run npm install: {}", e))?
        };
        if output.status.success() {
            results.push("Claude Code installed successfully".to_string());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to install Claude Code: {}", stderr));
        }
    }

    if install_copilot_api {
        let output = {
            let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.args(["/c", "npm", "install", "-g", "copilot-api"]);
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            #[cfg(not(windows))]
            {
                cmd.args(["-c", "npm install -g copilot-api"]);
            }
            cmd.output().map_err(|e| format!("Failed to run npm install: {}", e))?
        };
        if output.status.success() {
            results.push("Copilot API installed successfully".to_string());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to install Copilot API: {}", stderr));
        }
    }

    if install_copilot_cli {
        let output = {
            let mut cmd = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.args(["/c", "npm", "install", "-g", "@github/copilot"]);
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            #[cfg(not(windows))]
            {
                cmd.args(["-c", "npm install -g @github/copilot"]);
            }
            cmd.output().map_err(|e| format!("Failed to run npm install: {}", e))?
        };
        if output.status.success() {
            results.push("Copilot CLI installed successfully".to_string());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to install Copilot CLI: {}", stderr));
        }
    }

    Ok(results.join("; "))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init());

    // Single-instance guard is release-only.
    // In debug builds the app identifier is shared with any installed production
    // build, so the single-instance mutex would cause the dev window to exit
    // immediately whenever production is already running.
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));

    builder
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
                    let memory_dir = PathBuf::from(&tab.project_path).join(".node-memory");
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

            // Load persisted settings
            let settings = app_settings::load_settings(&app_data_dir);
            eprintln!("[workspace] Loaded settings: project_primary={}", settings.project_primary_model);

            app.manage(AppState {
                processes: ManagedProcesses::new(),
                logs: LogBuffer::new(),
                tabs: Mutex::new(runtime_tabs),
                active_tab: Mutex::new(store.active_tab_id.clone()),
                app_data_dir,
                tab_store: Mutex::new(store),
                settings: Mutex::new(settings),
                pending_auth_pid: Mutex::new(None),
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
            detect_project_memory,
            scaffold_project_cmd,
            sync_memory_structure,
            get_migration_file_contents,
            update_claude_md_references,
            close_tab,
            close_tab_by_id,
            close_all_tabs,
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
            // Settings
            get_app_settings,
            save_app_settings,
            get_available_models,
            get_copilot_models,
            scan_copilot_models,
            // Copilot auth
            get_copilot_auth_status,
            start_copilot_auth,
            cancel_copilot_auth,
            sign_out_copilot,
            // Dependency checks
            check_dependencies,
            install_dependencies,
            apply_cli_mode,
            sync_cli_files,
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
