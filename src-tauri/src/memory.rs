use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MemoryNode {
    pub id: String,
    #[serde(alias = "type")]
    pub node_type: String,
    pub l0: String,
    pub l1: String,
    pub l2: String,
    pub score: f64,
    pub last_touched: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MemoryEdge {
    pub from: String,
    pub to: String,
    pub rel: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MemoryGraph {
    pub nodes: Vec<MemoryNode>,
    pub edges: Vec<MemoryEdge>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MemoryState {
    pub active_task: String,
    pub branch: String,
    pub next_action: String,
}

/// Parse graph.toml into MemoryGraph
pub fn parse_graph(memory_dir: &Path) -> Result<MemoryGraph, String> {
    let graph_path = memory_dir.join("graph.toml");
    let content = std::fs::read_to_string(&graph_path)
        .map_err(|e| format!("Failed to read graph.toml: {}", e))?;

    let table: toml::Value = content
        .parse()
        .map_err(|e| format!("Failed to parse graph.toml: {}", e))?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if let Some(tbl) = table.as_table() {
        for (key, value) in tbl {
            if key.starts_with("N.") {
                if let Some(node_tbl) = value.as_table() {
                    nodes.push(MemoryNode {
                        id: key.clone(),
                        node_type: node_tbl
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        l0: node_tbl
                            .get("L0")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        l1: node_tbl
                            .get("L1")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        l2: node_tbl
                            .get("L2")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        score: node_tbl
                            .get("score")
                            .and_then(|v| v.as_float())
                            .unwrap_or(0.5),
                        last_touched: node_tbl
                            .get("last_touched")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }

        if let Some(edge_arr) = tbl.get("E").and_then(|v| v.as_array()) {
            for edge_val in edge_arr {
                if let Some(edge_tbl) = edge_val.as_table() {
                    edges.push(MemoryEdge {
                        from: edge_tbl
                            .get("from")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        to: edge_tbl
                            .get("to")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        rel: edge_tbl
                            .get("rel")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
    }

    Ok(MemoryGraph { nodes, edges })
}

/// Parse state.toml
pub fn parse_state(memory_dir: &Path) -> Result<MemoryState, String> {
    let state_path = memory_dir.join("state.toml");
    let content = std::fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read state.toml: {}", e))?;

    let table: toml::Value = content
        .parse()
        .map_err(|e| format!("Failed to parse state.toml: {}", e))?;

    Ok(MemoryState {
        active_task: table
            .get("active_task")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        branch: table
            .get("branch")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        next_action: table
            .get("next_action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Read a node file's raw content
pub fn read_node_file(memory_dir: &Path, node_id: &str) -> Result<String, String> {
    // node_id is like "N.app" → file is nodes/app.toml
    let name = node_id.strip_prefix("N.").unwrap_or(node_id);
    let path = memory_dir.join("nodes").join(format!("{}.toml", name));
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read node file: {}", e))
}

/// Start watching .node-memory/ and emit events on changes (legacy, no tab_id)
pub fn start_watcher(app: AppHandle, memory_dir: PathBuf) {
    start_watcher_for_tab(app, memory_dir, String::new());
}

/// Start watching a tab's .node-memory/ and emit events tagged with tab_id
pub fn start_watcher_for_tab(app: AppHandle, memory_dir: PathBuf, tab_id: String) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, Config::default().with_poll_interval(std::time::Duration::from_secs(2)))
                .expect("Failed to create file watcher");

        watcher
            .watch(&memory_dir, RecursiveMode::Recursive)
            .expect("Failed to watch memory directory");

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    let payload = serde_json::json!({
                        "tab_id": tab_id,
                        "kind": format!("{:?}", event.kind),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    let _ = app.emit("memory-changed", payload);
                }
                Ok(Err(e)) => {
                    eprintln!("Watch error: {}", e);
                }
                Err(_) => break,
            }
        }
    });
}
