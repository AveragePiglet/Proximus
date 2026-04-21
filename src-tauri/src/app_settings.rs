use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Persisted application settings (settings.json in app data dir).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AppSettings {
    /// Model used for project tabs — primary (big thinking / planning)
    #[serde(default = "default_project_primary")]
    pub project_primary_model: String,

    /// Model used for project tabs — secondary (coding / grunt work)
    #[serde(default = "default_project_secondary")]
    pub project_secondary_model: String,

    /// Model used for chat (scratch) tabs
    #[serde(default = "default_chat_model")]
    pub chat_model: String,

    /// When true, launch Claude with --dangerously-skip-permissions
    #[serde(default)]
    pub dangerously_skip_permissions: bool,
}

fn default_project_primary() -> String {
    "claude-opus-4-7".to_string()
}

fn default_project_secondary() -> String {
    "claude-sonnet-4-6".to_string()
}

fn default_chat_model() -> String {
    "claude-haiku-4-5".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            project_primary_model: default_project_primary(),
            project_secondary_model: default_project_secondary(),
            chat_model: default_chat_model(),
            dangerously_skip_permissions: false,
        }
    }
}

fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

pub fn load_settings(app_data_dir: &Path) -> AppSettings {
    let path = settings_path(app_data_dir);
    if !path.exists() {
        return AppSettings::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("[settings] Failed to parse settings.json: {}", e);
            AppSettings::default()
        }),
        Err(e) => {
            eprintln!("[settings] Failed to read settings.json: {}", e);
            AppSettings::default()
        }
    }
}

pub fn save_settings(app_data_dir: &Path, settings: &AppSettings) {
    let path = settings_path(app_data_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                eprintln!("[settings] Failed to write settings.json: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[settings] Failed to serialize settings: {}", e);
        }
    }
}

/// A model entry returned to the frontend.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelEntry {
    /// The model ID as passed to --model (e.g. "claude-opus-4-5")
    pub id: String,
    /// Human-readable display name (e.g. "Claude Opus 4.5")
    pub display_name: String,
    /// Tier: "opus" | "sonnet" | "haiku"
    pub tier: String,
}

/// Try to list available Claude models by running `claude models --json`.
/// Falls back to a hardcoded list of well-known current models on any error.
pub fn get_available_models() -> Vec<ModelEntry> {
    // Attempt to call the Claude CLI for a live model list
    let output = std::process::Command::new("claude")
        .args(["models", "--json"])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(models) = parse_claude_models_json(&stdout) {
                if !models.is_empty() {
                    eprintln!("[settings] Loaded {} model(s) from claude CLI", models.len());
                    return models;
                }
            }
        } else {
            eprintln!("[settings] claude models --json exited non-zero: {}",
                String::from_utf8_lossy(&out.stderr));
        }
    } else {
        eprintln!("[settings] claude CLI not found or failed — using fallback model list");
    }

    fallback_models()
}

/// Parse the JSON output of `claude models --json`.
/// The Claude CLI emits an array of objects; we look for id fields and classify by tier.
fn parse_claude_models_json(json: &str) -> Result<Vec<ModelEntry>, ()> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| ())?;

    let arr = match value.as_array() {
        Some(a) => a,
        // Some CLI versions wrap it: { "models": [...] }
        None => value.get("models").and_then(|v| v.as_array()).ok_or(())?,
    };

    let mut entries: Vec<ModelEntry> = arr
        .iter()
        .filter_map(|item| {
            let id = item.get("id")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())
                .map(str::to_string)?;

            // Only surface Claude models
            if !id.starts_with("claude-") {
                return None;
            }

            let tier = classify_tier(&id);
            let display_name = item
                .get("display_name")
                .or_else(|| item.get("displayName"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format_display_name(&id));

            Some(ModelEntry { id, display_name, tier })
        })
        .collect();

    // Sort: opus → sonnet → haiku, then by id descending (newest first)
    entries.sort_by(|a, b| {
        tier_order(&a.tier).cmp(&tier_order(&b.tier))
            .then(b.id.cmp(&a.id))
    });

    // Deduplicate by id
    entries.dedup_by(|a, b| a.id == b.id);

    Ok(entries)
}

fn classify_tier(id: &str) -> String {
    let lower = id.to_lowercase();
    if lower.contains("opus") {
        "opus".to_string()
    } else if lower.contains("haiku") {
        "haiku".to_string()
    } else {
        "sonnet".to_string()
    }
}

fn tier_order(tier: &str) -> u8 {
    match tier {
        "opus"   => 0,
        "sonnet" => 1,
        "haiku"  => 2,
        _        => 3,
    }
}

fn format_display_name(id: &str) -> String {
    // "claude-opus-4-5" → "Claude Opus 4.5"
    let stripped = id.strip_prefix("claude-").unwrap_or(id);
    let parts: Vec<&str> = stripped.split('-').collect();
    let mut out = String::from("Claude");
    let mut version_parts: Vec<String> = Vec::new();
    let mut in_version = false;
    for part in parts {
        if part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            version_parts.push(part.to_string());
            in_version = true;
        } else if !in_version {
            let mut chars = part.chars();
            let capitalized = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()
                + chars.as_str();
            out.push(' ');
            out.push_str(&capitalized);
        }
    }
    if !version_parts.is_empty() {
        out.push(' ');
        out.push_str(&version_parts.join("."));
    }
    out
}

/// Hardcoded fallback — updated to current Claude model lineup.
/// Used when the CLI is unavailable or returns nothing.
/// Keep in sync with https://docs.anthropic.com/en/docs/about-claude/models
fn fallback_models() -> Vec<ModelEntry> {
    vec![
        ModelEntry {
            id: "claude-opus-4-7".to_string(),
            display_name: "Claude Opus 4.7".to_string(),
            tier: "opus".to_string(),
        },
        ModelEntry {
            id: "claude-opus-4-6".to_string(),
            display_name: "Claude Opus 4.6".to_string(),
            tier: "opus".to_string(),
        },
        ModelEntry {
            id: "claude-sonnet-4-6".to_string(),
            display_name: "Claude Sonnet 4.6".to_string(),
            tier: "sonnet".to_string(),
        },
        ModelEntry {
            id: "claude-haiku-4-5".to_string(),
            display_name: "Claude Haiku 4.5".to_string(),
            tier: "haiku".to_string(),
        },
    ]
}
