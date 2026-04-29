//! Claude ↔ Copilot file structure sync.
//!
//! Claude layout:
//!   {project}/CLAUDE.md
//!   {project}/.claude/skills/*/SKILL.md
//!   {project}/.node-memory/  (tool-agnostic — kept as-is)
//!
//! Copilot layout:
//!   {project}/.github/copilot-instructions.md
//!   {project}/.github/prompts/*.prompt.md
//!   {project}/.node-memory/  (unchanged)

use std::fs;
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SyncResult {
    pub files_written: Vec<String>,
    pub files_skipped: Vec<String>,
}

pub fn sync_cli_files(
    project_path: &str,
    from_mode: &str,
    to_mode: &str,
) -> Result<SyncResult, String> {
    let root = Path::new(project_path);
    let mut result = SyncResult {
        files_written: Vec::new(),
        files_skipped: Vec::new(),
    };

    match (from_mode, to_mode) {
        ("claude", "copilot") => sync_claude_to_copilot(root, &mut result)?,
        ("copilot", "claude") => sync_copilot_to_claude(root, &mut result)?,
        _ => return Err(format!("Unsupported sync direction: {} → {}", from_mode, to_mode)),
    }

    Ok(result)
}

// ── Claude → Copilot ────────────────────────────────────────────

fn sync_claude_to_copilot(root: &Path, result: &mut SyncResult) -> Result<(), String> {
    let github_dir = root.join(".github");
    ensure_dir(&github_dir)?;
    let prompts_dir = github_dir.join("prompts");
    ensure_dir(&prompts_dir)?;

    // CLAUDE.md → .github/copilot-instructions.md
    let claude_md = root.join("CLAUDE.md");
    if claude_md.is_file() {
        let content = fs::read_to_string(&claude_md)
            .map_err(|e| format!("Failed to read CLAUDE.md: {}", e))?;
        let translated = translate_claude_to_copilot(&content);
        write_file(&github_dir.join("copilot-instructions.md"), &translated)?;
        result.files_written.push(".github/copilot-instructions.md".into());
    } else {
        result.files_skipped.push("CLAUDE.md (not found)".into());
    }

    // .claude/skills/*/SKILL.md → .github/prompts/{name}.prompt.md
    let skills_dir = root.join(".claude").join("skills");
    if skills_dir.is_dir() {
        for entry in fs::read_dir(&skills_dir)
            .map_err(|e| format!("Failed to read .claude/skills: {}", e))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let skill_dir = entry.path();
            if !skill_dir.is_dir() { continue; }
            let skill_name = skill_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let skill_md = skill_dir.join("SKILL.md");
            if skill_md.is_file() {
                let content = fs::read_to_string(&skill_md)
                    .map_err(|e| format!("Failed to read {}: {}", skill_md.display(), e))?;
                let dest = prompts_dir.join(format!("{}.prompt.md", skill_name));
                write_file(&dest, &translate_skill_to_prompt(&skill_name, &content))?;
                result.files_written.push(format!(".github/prompts/{}.prompt.md", skill_name));
            }
        }
    }

    Ok(())
}

// ── Copilot → Claude ────────────────────────────────────────────

fn sync_copilot_to_claude(root: &Path, result: &mut SyncResult) -> Result<(), String> {
    let skills_dir = root.join(".claude").join("skills");

    // .github/copilot-instructions.md → CLAUDE.md
    let copilot_instructions = root.join(".github").join("copilot-instructions.md");
    if copilot_instructions.is_file() {
        let content = fs::read_to_string(&copilot_instructions)
            .map_err(|e| format!("Failed to read copilot-instructions.md: {}", e))?;
        write_file(&root.join("CLAUDE.md"), &translate_copilot_to_claude(&content))?;
        result.files_written.push("CLAUDE.md".into());
    } else {
        result.files_skipped.push(".github/copilot-instructions.md (not found)".into());
    }

    // .github/prompts/*.prompt.md → .claude/skills/{name}/SKILL.md
    let prompts_dir = root.join(".github").join("prompts");
    if prompts_dir.is_dir() {
        for entry in fs::read_dir(&prompts_dir)
            .map_err(|e| format!("Failed to read .github/prompts: {}", e))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .trim_end_matches(".prompt")
                .to_string();
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let skill_dir = skills_dir.join(&stem);
            ensure_dir(&skill_dir)?;
            write_file(&skill_dir.join("SKILL.md"), &translate_prompt_to_skill(&stem, &content))?;
            result.files_written.push(format!(".claude/skills/{}/SKILL.md", stem));
        }
    }

    Ok(())
}

// ── Translation helpers ─────────────────────────────────────────

fn translate_claude_to_copilot(content: &str) -> String {
    let mut out = String::from("<!-- Imported from CLAUDE.md — review for Copilot conventions -->\n\n");
    for line in content.lines() {
        // Skip import comment headers from previous syncs
        let trimmed = line.trim();
        if trimmed.starts_with("<!-- Imported from") && trimmed.ends_with("-->") { continue; }
        // Strip bare slash commands (e.g. /memory, /init on their own line)
        if trimmed.starts_with('/') && !trimmed.contains(' ') { continue; }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn translate_copilot_to_claude(content: &str) -> String {
    let mut out = String::from("# Project Memory Protocol\n\n<!-- Imported from .github/copilot-instructions.md — review for Claude conventions -->\n\n");
    for line in content.lines() {
        let trimmed = line.trim();
        // Skip import comment headers from previous syncs
        if trimmed.starts_with("<!-- Imported from") && trimmed.ends_with("-->") { continue; }
        // Skip duplicate "# Project Memory Protocol" headings
        if trimmed == "# Project Memory Protocol" { continue; }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn translate_skill_to_prompt(name: &str, content: &str) -> String {
    // Strip any existing import headers before re-wrapping (idempotent)
    let body = strip_import_headers(content);
    // Strip YAML frontmatter if already present
    let body = if body.trim_start().starts_with("---\n") {
        match body[4..].find("\n---\n") {
            Some(end) => body[4 + end + 5..].to_string(),
            None => body,
        }
    } else {
        body
    };
    format!(
        "---\ndescription: \"Skill: {}\"\nmode: agent\n---\n\n<!-- Imported from .claude/skills/ -->\n\n{}",
        name, body.trim_start()
    )
}

fn translate_prompt_to_skill(name: &str, content: &str) -> String {
    // Strip YAML frontmatter if present
    let body = if content.starts_with("---\n") {
        match content[4..].find("\n---\n") {
            Some(end) => &content[4 + end + 5..],
            None => content,
        }
    } else {
        content
    };
    // Strip any existing import headers before re-wrapping (idempotent)
    let body = strip_import_headers(body);
    format!(
        "# {} Skill\n\n<!-- Imported from .github/prompts/ -->\n\n{}",
        name, body.trim_start()
    )
}

/// Remove all `<!-- Imported from ... -->` comment lines and the
/// `# <name> Skill` header line that the sync prepends, so repeated
/// syncs don't keep accumulating duplicate headers.
fn strip_import_headers(content: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut skip_blank_after_header = false;

    for line in content.lines() {
        let trimmed = line.trim();
        // Drop import comment lines
        if trimmed.starts_with("<!-- Imported from") && trimmed.ends_with("-->") {
            skip_blank_after_header = true;
            continue;
        }
        // Drop "# <name> Skill" heading lines
        if trimmed.starts_with("# ") && trimmed.ends_with(" Skill") {
            skip_blank_after_header = true;
            continue;
        }
        // Drop one blank line immediately after a stripped header
        if skip_blank_after_header && trimmed.is_empty() {
            skip_blank_after_header = false;
            continue;
        }
        skip_blank_after_header = false;
        lines.push(line);
    }

    lines.join("\n")
}

// ── Utilities ────────────────────────────────────────────────────

fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create dir {}: {}", path.display(), e))?;
    }
    Ok(())
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}
