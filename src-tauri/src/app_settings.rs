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

    /// CLI mode: "claude" (default) uses Claude CLI + Copilot Proxy,
    /// "copilot" launches the Copilot CLI directly without proxies.
    #[serde(default = "default_cli_mode")]
    pub cli_mode: String,

    /// Model passed to `copilot --model` when cli_mode is "copilot".
    #[serde(default = "default_copilot_model")]
    pub copilot_model: String,
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

fn default_cli_mode() -> String {
    "claude".to_string()
}

fn default_copilot_model() -> String {
    "gpt-5.4".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            project_primary_model: default_project_primary(),
            project_secondary_model: default_project_secondary(),
            chat_model: default_chat_model(),
            dangerously_skip_permissions: false,
            cli_mode: default_cli_mode(),
            copilot_model: default_copilot_model(),
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

fn parse_claude_models_json(json: &str) -> Result<Vec<ModelEntry>, ()> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| ())?;

    let arr = match value.as_array() {
        Some(a) => a,
        None => value.get("models").and_then(|v| v.as_array()).ok_or(())?,
    };

    let mut entries: Vec<ModelEntry> = arr
        .iter()
        .filter_map(|item| {
            let id = item.get("id")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())
                .map(str::to_string)?;

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

    entries.sort_by(|a, b| {
        tier_order(&a.tier).cmp(&tier_order(&b.tier))
            .then(b.id.cmp(&a.id))
    });

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

/// Hardcoded list of models supported by the GitHub Copilot CLI.
/// Copilot doesn't expose a --models JSON command, so we maintain this manually.
pub fn get_copilot_models() -> Vec<ModelEntry> {
    vec![
        // GPT-5 family
        ModelEntry { id: "gpt-5.4".to_string(),              display_name: "GPT-5.4".to_string(),              tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.4-mini".to_string(),         display_name: "GPT-5.4 Mini".to_string(),         tier: "sonnet".to_string() },
        ModelEntry { id: "gpt-5.4-nano".to_string(),         display_name: "GPT-5.4 Nano".to_string(),         tier: "haiku".to_string() },
        ModelEntry { id: "gpt-5.2".to_string(),              display_name: "GPT-5.2".to_string(),              tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.1".to_string(),              display_name: "GPT-5.1".to_string(),              tier: "opus".to_string() },
        ModelEntry { id: "gpt-5".to_string(),                display_name: "GPT-5".to_string(),                tier: "opus".to_string() },
        ModelEntry { id: "gpt-5-mini".to_string(),           display_name: "GPT-5 Mini".to_string(),           tier: "haiku".to_string() },
        // Codex family
        ModelEntry { id: "gpt-5.4-codex".to_string(),        display_name: "GPT-5.4 Codex".to_string(),        tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.3-codex".to_string(),        display_name: "GPT-5.3 Codex".to_string(),        tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.2-codex".to_string(),        display_name: "GPT-5.2 Codex".to_string(),        tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.1-codex".to_string(),        display_name: "GPT-5.1 Codex".to_string(),        tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.1-codex-max".to_string(),    display_name: "GPT-5.1 Codex Max".to_string(),    tier: "opus".to_string() },
        ModelEntry { id: "gpt-5.1-codex-mini".to_string(),   display_name: "GPT-5.1 Codex Mini".to_string(),   tier: "sonnet".to_string() },
        ModelEntry { id: "gpt-5-codex".to_string(),          display_name: "GPT-5 Codex".to_string(),          tier: "opus".to_string() },
        ModelEntry { id: "codex".to_string(),                display_name: "Codex".to_string(),                tier: "sonnet".to_string() },
        // GPT-4 family
        ModelEntry { id: "gpt-4.1".to_string(),              display_name: "GPT-4.1".to_string(),              tier: "sonnet".to_string() },
        ModelEntry { id: "gpt-4o".to_string(),               display_name: "GPT-4o".to_string(),               tier: "sonnet".to_string() },
        ModelEntry { id: "gpt-4o-mini".to_string(),          display_name: "GPT-4o Mini".to_string(),          tier: "haiku".to_string() },
        // Claude family
        ModelEntry { id: "claude-opus-4.7".to_string(),      display_name: "Claude Opus 4.7".to_string(),      tier: "opus".to_string() },
        ModelEntry { id: "claude-sonnet-4.6".to_string(),    display_name: "Claude Sonnet 4.6".to_string(),    tier: "sonnet".to_string() },
        ModelEntry { id: "claude-sonnet-4-5".to_string(),    display_name: "Claude Sonnet 4.5".to_string(),    tier: "sonnet".to_string() },
        // Gemini family
        ModelEntry { id: "gemini-2.5-pro".to_string(),       display_name: "Gemini 2.5 Pro".to_string(),       tier: "opus".to_string() },
        ModelEntry { id: "gemini-3.1-pro-preview".to_string(), display_name: "Gemini 3.1 Pro Preview".to_string(), tier: "opus".to_string() },
    ]
}

/// Try to extract model IDs from the installed Copilot CLI's bundled app.js.
/// Falls back to the hardcoded list if the file can't be found or parsed.
pub fn scan_copilot_models() -> Vec<ModelEntry> {
    let candidates = copilot_app_js_candidates();
    for path in &candidates {
        if !path.exists() { continue; }
        let Ok(src) = std::fs::read_to_string(path) else { continue };

        // Extract quoted strings that look like model IDs
        let re_pat = r#""((?:gpt|o[0-9]|codex|gemini|claude)[a-z0-9._-]*)""#;
        let re = regex::Regex::new(re_pat).unwrap();
        let mut seen = std::collections::HashSet::new();
        let mut models: Vec<ModelEntry> = re
            .captures_iter(&src)
            .filter_map(|cap| {
                let id = cap[1].to_string();
                // Filter out obvious false positives (too short, version strings, etc.)
                if id.len() < 3 || id.ends_with('-') { return None; }
                if seen.contains(&id) { return None; }
                seen.insert(id.clone());
                let display_name = format_copilot_model_name(&id);
                let tier = classify_copilot_tier(&id);
                Some(ModelEntry { id, display_name, tier })
            })
            .collect();

        if models.len() > 3 {
            // Sort: opus first, then sonnet, then haiku, then alpha within tier
            models.sort_by(|a, b| {
                tier_order(&a.tier).cmp(&tier_order(&b.tier)).then(b.id.cmp(&a.id))
            });
            eprintln!("[copilot-models] Scanned {} models from {:?}", models.len(), path);
            return models;
        }
    }

    eprintln!("[copilot-models] app.js not found — using hardcoded list");
    get_copilot_models()
}

fn copilot_app_js_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    // npm global install: %APPDATA%/npm/node_modules/@github/copilot/app.js
    if let Ok(appdata) = std::env::var("APPDATA") {
        v.push(std::path::PathBuf::from(&appdata)
            .join("npm").join("node_modules").join("@github").join("copilot").join("app.js"));
    }
    // Cached pkg version (Windows): %LOCALAPPDATA%/copilot/pkg/win32-x64/<version>/app.js
    if let Some(local) = dirs::data_local_dir() {
        let pkg_dir = local.join("copilot").join("pkg");
        if let Ok(rd) = std::fs::read_dir(&pkg_dir) {
            // Flatten arch subdirs, then version subdirs
            for arch in rd.flatten() {
                if let Ok(versions) = std::fs::read_dir(arch.path()) {
                    for ver in versions.flatten() {
                        v.push(ver.path().join("app.js"));
                    }
                }
            }
        }
    }
    // macOS/Linux npm global
    if let Some(home) = dirs::home_dir() {
        v.push(home.join(".npm-global").join("lib").join("node_modules")
            .join("@github").join("copilot").join("app.js"));
    }
    // /usr/local/lib/node_modules
    v.push(std::path::PathBuf::from("/usr/local/lib/node_modules")
        .join("@github").join("copilot").join("app.js"));
    v
}

fn format_copilot_model_name(id: &str) -> String {
    // e.g. "gpt-5.4-codex-mini" → "GPT-5.4 Codex Mini"
    // "claude-sonnet-4.6" → "Claude Sonnet 4.6"
    // "gemini-2.5-pro" → "Gemini 2.5 Pro"
    let prefix_map = [("gpt-", "GPT-"), ("o3", "o3"), ("o4", "o4"), ("o2", "o2"),
                      ("codex", "Codex"), ("gemini-", "Gemini "), ("claude-", "Claude ")];
    for (pfx, label) in &prefix_map {
        if id.starts_with(pfx) {
            let rest = &id[pfx.len()..];
            let rest_pretty: String = rest.split('-')
                .map(|p| { let mut c = p.chars(); c.next().map(|f| f.to_uppercase().to_string() + c.as_str()).unwrap_or_default() })
                .collect::<Vec<_>>().join(" ");
            return format!("{}{}", label, rest_pretty);
        }
    }
    id.to_string()
}

fn classify_copilot_tier(id: &str) -> String {
    let l = id.to_lowercase();
    if l.contains("nano") || l.contains("mini") || l.contains("haiku") || l == "o4-mini" {
        "haiku".to_string()
    } else if l.contains("opus") || l.contains("max") || l.starts_with("gpt-5")
           || l.starts_with("o3") || l.starts_with("o2") || l.contains("gemini") {
        "opus".to_string()
    } else {
        "sonnet".to_string()
    }
}
