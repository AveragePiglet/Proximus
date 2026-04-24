use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TabState {
    pub id: String,
    pub project_path: String,
    pub project_name: String,
    pub status: String, // "active" | "closed"
    pub tab_type: String, // "project" | "chat"
    pub last_opened: String,
    pub created_at: String,
    #[serde(default)]
    pub pty_started_at: Option<String>,
    #[serde(default)]
    pub last_memory_save: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TabStore {
    pub tabs: Vec<TabState>,
    pub active_tab_id: Option<String>,
}

impl TabStore {
    pub fn empty() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }
}

fn store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("tabs.json")
}

fn now_iso() -> String {
    // Simple timestamp without chrono dependency
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}

pub fn load_tabs(app_data_dir: &Path) -> TabStore {
    let path = store_path(app_data_dir);
    if !path.exists() {
        return TabStore::empty();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("[tab_store] Failed to parse tabs.json: {}", e);
            TabStore::empty()
        }),
        Err(e) => {
            eprintln!("[tab_store] Failed to read tabs.json: {}", e);
            TabStore::empty()
        }
    }
}

pub fn save_tabs(app_data_dir: &Path, store: &TabStore) {
    let path = store_path(app_data_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(store) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                eprintln!("[tab_store] Failed to write tabs.json: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[tab_store] Failed to serialize tabs: {}", e);
        }
    }
}

pub fn add_tab(store: &mut TabStore, id: String, project_path: String, tab_type: &str) {
    let project_name = Path::new(&project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_path.clone());

    let ts = now_iso();

    store.tabs.push(TabState {
        id: id.clone(),
        project_path,
        project_name,
        status: "active".into(),
        tab_type: tab_type.into(),
        last_opened: ts.clone(),
        created_at: ts,
        pty_started_at: None,
        last_memory_save: None,
    });
    store.active_tab_id = Some(id);
}

pub fn close_tab(store: &mut TabStore, tab_id: &str) {
    if let Some(tab) = store.tabs.iter_mut().find(|t| t.id == tab_id) {
        tab.status = "closed".into();
    }
    if store.active_tab_id.as_deref() == Some(tab_id) {
        // Switch to next active tab
        store.active_tab_id = store
            .tabs
            .iter()
            .find(|t| t.status == "active" && t.id != tab_id)
            .map(|t| t.id.clone());
    }
}

pub fn reopen_tab(store: &mut TabStore, tab_id: &str) {
    if let Some(tab) = store.tabs.iter_mut().find(|t| t.id == tab_id) {
        tab.status = "active".into();
        tab.last_opened = now_iso();
    }
    store.active_tab_id = Some(tab_id.to_string());
}

pub fn remove_tab(store: &mut TabStore, tab_id: &str) {
    store.tabs.retain(|t| t.id != tab_id);
    if store.active_tab_id.as_deref() == Some(tab_id) {
        store.active_tab_id = store
            .tabs
            .iter()
            .find(|t| t.status == "active")
            .map(|t| t.id.clone());
    }
}

pub fn update_tab_stats(store: &mut TabStore, tab_id: &str, pty_started_at: Option<String>, last_memory_save: Option<String>) {
    if let Some(tab) = store.tabs.iter_mut().find(|t| t.id == tab_id) {
        if pty_started_at.is_some() {
            tab.pty_started_at = pty_started_at;
        }
        if last_memory_save.is_some() {
            tab.last_memory_save = last_memory_save;
        }
    }
}

/// Mark every active tab as closed and clear the active tab pointer.
pub fn close_all(store: &mut TabStore) {
    for tab in store.tabs.iter_mut() {
        if tab.status == "active" {
            tab.status = "closed".into();
        }
    }
    store.active_tab_id = None;
}
