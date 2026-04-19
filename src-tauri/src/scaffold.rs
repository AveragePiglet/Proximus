use include_dir::{include_dir, Dir};
use std::fs;
use std::io::Read;
use std::path::Path;

/// The template directory is embedded at compile time.
static TEMPLATE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/project-scaffold");

/// Scaffold a new project by extracting embedded template files.
/// Returns Ok(true) if scaffolding was performed, Ok(false) if already exists.
pub fn scaffold_project(project_path: &str) -> Result<bool, String> {
    let root = Path::new(project_path);
    if !root.exists() {
        return Err(format!("Project path does not exist: {}", project_path));
    }

    let memory_dir = root.join(".claude-memory");

    // Already has memory system — skip
    if memory_dir.exists() {
        eprintln!("[scaffold] .claude-memory/ already exists at {:?}", memory_dir);
        return Ok(false);
    }

    eprintln!("[scaffold] Scaffolding project at {:?}", root);

    extract_dir(&TEMPLATE_DIR, root)?;

    // Ensure empty dirs exist (include_dir skips empty directories)
    for dir in &[".claude-memory/nodes", ".claude-memory/journal", ".claude"] {
        let p = root.join(dir);
        if !p.exists() {
            fs::create_dir_all(&p)
                .map_err(|e| format!("Failed to create {}: {}", dir, e))?;
        }
    }

    eprintln!("[scaffold] Project scaffolded successfully at {:?}", root);
    Ok(true)
}

/// Detect existing AI memory/context files in a project.
/// Returns a list of relative paths to detected memory files/dirs.
/// Empty vec means this is a fresh project with no existing memory.
pub fn detect_existing_memory(project_path: &str) -> Result<Vec<String>, String> {
    let root = Path::new(project_path);
    if !root.exists() {
        return Err(format!("Project path does not exist: {}", project_path));
    }

    // If .claude-memory/MANIFEST.toml already exists, migration is done — skip
    if root.join(".claude-memory").join("MANIFEST.toml").exists() {
        return Ok(Vec::new());
    }

    let mut found: Vec<String> = Vec::new();

    // Files to check (must have meaningful content)
    let content_files = [
        "CLAUDE.md",
        "MEMORY.md",
        "CONTEXT.md",
        "PROJECT_CONTEXT.md",
        "AGENTS.md",
        "CODEX.md",
        ".cursorrules",
        ".copilot-instructions.md",
        ".github/copilot-instructions.md",
        "docs/architecture.md",
    ];

    for rel in &content_files {
        let p = root.join(rel);
        if p.is_file() {
            // Only count files with >10 lines (skip near-empty stubs)
            if let Ok(mut f) = fs::File::open(&p) {
                let mut buf = String::new();
                if f.read_to_string(&mut buf).is_ok() && buf.lines().count() > 10 {
                    found.push(rel.to_string());
                }
            }
        }
    }

    // Directories to check (existence alone is enough)
    let dirs = [
        "memory",
        ".claude/memory",
        ".claude/notes",
        ".cursor/rules",
        ".ai",
        ".aider",
        ".continue",
        "docs/decisions",
    ];

    for rel in &dirs {
        let p = root.join(rel);
        if p.is_dir() {
            found.push(rel.to_string());
        }
    }

    Ok(found)
}

/// Memory protocol block to inject into CLAUDE.md
const MEMORY_PROTOCOL_HEADER: &str = r#"# Project Memory Protocol

This project uses a graph-based memory system at `.claude-memory/`.

## Session start
1. Read `.claude-memory/MANIFEST.toml` — vocab and load order
2. Read `.claude-memory/invariants.toml` — absolute rules
3. Read `.claude-memory/state.toml` — current task
4. Read `.claude-memory/graph.toml` — L0/L1 summaries of all nodes
5. Load `nodes/<name>.toml` only when working in that domain
"#;

/// Update CLAUDE.md in the project to replace old memory references with .claude-memory/ paths.
/// Strips old memory/context sections and injects the new memory protocol header.
pub fn update_claude_md_references(project_path: &str) -> Result<bool, String> {
    let root = Path::new(project_path);
    let claude_md = root.join("CLAUDE.md");

    if !claude_md.is_file() {
        return Ok(false);
    }

    let content = fs::read_to_string(&claude_md)
        .map_err(|e| format!("Failed to read CLAUDE.md: {}", e))?;

    // If already migrated, skip
    if content.contains(".claude-memory/MANIFEST.toml") {
        return Ok(false);
    }

    let mut updated = content.clone();

    // Replace common old memory path references (order matters — longer paths first)
    let replacements = [
        (".claude/memory", ".claude-memory"),
        (".claude/notes", ".claude-memory/nodes"),
        ("memory/", ".claude-memory/"),
        ("MEMORY.md", ".claude-memory/graph.toml"),
        ("CONTEXT.md", ".claude-memory/state.toml"),
        ("PROJECT_CONTEXT.md", ".claude-memory/graph.toml"),
        (".cursor/rules", ".claude-memory"),
        (".cursorrules", ".claude-memory"),
    ];

    for (old, new) in &replacements {
        updated = updated.replace(old, new);
    }

    // Remove entire sections about old memory system (between ## Memory headings and next ##)
    let section_headers_to_remove = [
        "## Memory & Context",
        "## Memory and Context",
        "## Memory System",
        "## Context & Memory",
    ];

    for header in &section_headers_to_remove {
        if let Some(start) = updated.find(header) {
            // Find the next ## heading after this section (or end of file)
            let after_header = start + header.len();
            let end = updated[after_header..]
                .find("\n## ")
                .map(|pos| after_header + pos)
                .unwrap_or(updated.len());
            updated = format!("{}{}", &updated[..start], &updated[end..]);
        }
    }

    // Remove lines referencing old memory update instructions
    let remove_patterns = [
        "update whichever `memory/`",
        "update whichever `.claude-memory/`",
        "must update memory",
        "Claude-Copilot.bat",
        "copilot-api proxy",
        "Run `Claude-Copilot.bat`",
    ];

    let lines: Vec<&str> = updated.lines().collect();
    let filtered: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            !remove_patterns.iter().any(|p| line.contains(p))
        })
        .collect();
    updated = filtered.join("\n");

    // Clean up excessive blank lines (3+ in a row → 2)
    while updated.contains("\n\n\n") {
        updated = updated.replace("\n\n\n", "\n\n");
    }

    // Prepend the memory protocol header
    updated = format!("{}\n{}", MEMORY_PROTOCOL_HEADER, updated.trim_start());

    if updated.trim() != content.trim() {
        fs::write(&claude_md, &updated)
            .map_err(|e| format!("Failed to write CLAUDE.md: {}", e))?;
        return Ok(true);
    }

    Ok(false)
}

/// Recursively extract an embedded Dir to disk, skipping existing files.
fn extract_dir(dir: &Dir, dst: &Path) -> Result<(), String> {
    for file in dir.files() {
        let dst_path = dst.join(file.path());
        if dst_path.exists() {
            continue;
        }
        // Skip .gitkeep files — they're only for git tracking
        if file.path().file_name().map(|n| n == ".gitkeep").unwrap_or(false) {
            continue;
        }
        if let Some(parent) = dst_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
            }
        }
        fs::write(&dst_path, file.contents())
            .map_err(|e| format!("Failed to write {:?}: {}", dst_path, e))?;
    }

    for subdir in dir.dirs() {
        let dst_subdir = dst.join(subdir.path());
        if !dst_subdir.exists() {
            fs::create_dir_all(&dst_subdir)
                .map_err(|e| format!("Failed to create dir {:?}: {}", dst_subdir, e))?;
        }
        extract_dir(subdir, dst)?;
    }

    Ok(())
}
