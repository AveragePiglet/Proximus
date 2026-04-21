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

    let memory_dir = root.join(".node-memory");

    // Already has memory system — skip
    if memory_dir.exists() {
        eprintln!("[scaffold] .node-memory/ already exists at {:?}", memory_dir);
        return Ok(false);
    }

    eprintln!("[scaffold] Scaffolding project at {:?}", root);

    extract_dir(&TEMPLATE_DIR, root)?;

    // Ensure empty dirs exist (include_dir skips empty directories)
    for dir in &[".node-memory/nodes", ".node-memory/journal", ".node-memory/plans", ".node-memory/cold", ".node-memory/tools", ".node-memory/prompts", ".claude"] {
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

    // If .node-memory/MANIFEST.toml already exists, migration is done — skip
    if root.join(".node-memory").join("MANIFEST.toml").exists() {
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

This project uses a graph-based memory system at `.node-memory/`.

## Session start
1. Read `.node-memory/MANIFEST.toml` — vocab and load order
2. Read `.node-memory/invariants.toml` — absolute rules
3. Read `.node-memory/state.toml` — current task
4. Read `.node-memory/graph.toml` — L0/L1 summaries of all nodes
5. Load `nodes/<name>.toml` only when working in that domain
"#;

/// Update CLAUDE.md in the project to replace old memory references with .node-memory/ paths.
/// Strips old memory/context sections and injects the new memory protocol header.
pub fn update_claude_md_references(project_path: &str) -> Result<bool, String> {
    let root = Path::new(project_path);
    let claude_md = root.join("CLAUDE.md");

    if !claude_md.is_file() {
        return Ok(false);
    }

    let content = fs::read_to_string(&claude_md)
        .map_err(|e| format!("Failed to read CLAUDE.md: {}", e))?;

    // If already migrated to .node-memory, skip
    if content.contains(".node-memory/MANIFEST.toml") {
        return Ok(false);
    }

    let mut updated = content.clone();

    // Replace common old memory path references (order matters — longer paths first)
    // Also covers legacy .claude-memory references from the old folder name
    let replacements = [
        (".claude/memory", ".node-memory"),
        (".claude/notes", ".node-memory/nodes"),
        ("memory/", ".node-memory/"),
        ("MEMORY.md", ".node-memory/graph.toml"),
        ("CONTEXT.md", ".node-memory/state.toml"),
        ("PROJECT_CONTEXT.md", ".node-memory/graph.toml"),
        (".cursor/rules", ".node-memory"),
        (".cursorrules", ".node-memory"),
        (".claude-memory/", ".node-memory/"),
        (".claude-memory", ".node-memory"),
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
        "update whichever `.node-memory/`",
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

/// Ensure an existing .node-memory/ directory has all expected subdirs and TOML files.
/// Compares the project's memory folder against the embedded template and creates anything
/// that is missing. Returns the list of relative paths that were added.
/// Returns an empty vec when nothing was missing (no-op for up-to-date projects).
pub fn ensure_memory_structure(project_path: &str) -> Result<Vec<String>, String> {
    let root = Path::new(project_path);
    let memory_dir = root.join(".node-memory");

    if !memory_dir.exists() {
        return Ok(Vec::new());
    }

    let mut added: Vec<String> = Vec::new();

    // Expected subdirectories — extend this list as the template grows
    let expected_dirs = ["nodes", "journal", "plans", "cold", "tools", "prompts"];
    for dir_name in &expected_dirs {
        let p = memory_dir.join(dir_name);
        if !p.exists() {
            fs::create_dir_all(&p)
                .map_err(|e| format!("Failed to create .node-memory/{}: {}", dir_name, e))?;
            added.push(format!(".node-memory/{}/", dir_name));
            eprintln!("[scaffold] Added missing dir: .node-memory/{}", dir_name);
        }
    }

    // Expected files — seed missing ones from the embedded template.
    // Paths are relative to the template root (project-scaffold/).
    let expected_files = [
        ".node-memory/MANIFEST.toml",
        ".node-memory/state.toml",
        ".node-memory/invariants.toml",
        ".node-memory/graph.toml",
        ".node-memory/tools/validate.py",
        ".node-memory/prompts/bootstrap.md",
        ".node-memory/prompts/bootstrap-repo.md",
        ".node-memory/prompts/claude-md-protocol.md",
        ".node-memory/prompts/migrate.md",
    ];
    for template_path in &expected_files {
        let rel = template_path.trim_start_matches(".node-memory/");
        let dest = memory_dir.join(rel);
        if !dest.exists() {
            // Ensure parent dir exists (e.g. tools/, prompts/)
            if let Some(parent) = dest.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
                }
            }
            let content = TEMPLATE_DIR
                .get_file(template_path)
                .map(|f| f.contents())
                .unwrap_or(b"");
            fs::write(&dest, content)
                .map_err(|e| format!("Failed to write {}: {}", template_path, e))?;
            added.push(template_path.to_string());
            eprintln!("[scaffold] Added missing file: {}", template_path);
        }
    }

    if !added.is_empty() {
        eprintln!("[scaffold] Structure sync complete — added {} item(s)", added.len());
    }

    Ok(added)
}

/// Migrate a project's legacy .claude-memory/ folder to .node-memory/.
/// Returns Ok(true) if migration was performed, Ok(false) if not needed.
pub fn migrate_legacy_memory(project_path: &str) -> Result<bool, String> {
    let root = Path::new(project_path);
    let old_dir = root.join(".claude-memory");
    let new_dir = root.join(".node-memory");

    // Nothing to migrate
    if !old_dir.exists() {
        return Ok(false);
    }

    // Already migrated
    if new_dir.exists() {
        return Ok(false);
    }

    eprintln!("[scaffold] Migrating .claude-memory → .node-memory at {:?}", root);

    // Attempt atomic rename first (works on same filesystem/drive)
    match fs::rename(&old_dir, &new_dir) {
        Ok(_) => {}
        Err(_) => {
            // Fallback: copy then delete (cross-device or permission issue)
            copy_dir_all(&old_dir, &new_dir)
                .map_err(|e| format!("Failed to copy .claude-memory to .node-memory: {}", e))?;
            fs::remove_dir_all(&old_dir)
                .map_err(|e| format!("Failed to remove old .claude-memory: {}", e))?;
        }
    }

    // Update any .claude-memory references inside the moved CLAUDE.md
    let _ = update_claude_md_references(project_path);

    eprintln!("[scaffold] Migration complete at {:?}", root);
    Ok(true)
}

/// Recursively copy a directory tree.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
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
