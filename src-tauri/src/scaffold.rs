use include_dir::{include_dir, Dir};
use std::fs;
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
