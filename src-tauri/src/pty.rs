use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct PtyState {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    // Must keep child and master alive or the PTY closes
    _child: Mutex<Box<dyn Child + Send + Sync>>,
    _master: Mutex<Box<dyn MasterPty + Send>>,
}

#[derive(Clone, Serialize)]
struct PtyOutputEvent {
    tab_id: String,
    data: String,
}

pub fn spawn_pty(app: &AppHandle, tab_id: &str, project_dir: &str, rewriter_port: u16, has_memory: bool, model: Option<String>, fallback_model: Option<String>, dangerous_mode: bool, cli_mode: &str, copilot_model: Option<String>) -> Result<PtyState, String> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let shell = if cfg!(windows) { "cmd" } else { "bash" };
    let mut cmd = CommandBuilder::new(shell);
    // Only set proxy env vars in claude mode
    if cli_mode == "claude" {
        cmd.env("ANTHROPIC_BASE_URL", format!("http://localhost:{}", rewriter_port));
        cmd.env("DISABLE_NON_ESSENTIAL_MODEL_CALLS", "1");
        cmd.env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
    }
    // In copilot mode, inject GitHub token so the CLI doesn't prompt for /login
    if cli_mode == "copilot" {
        if let Some(token) = read_github_token() {
            cmd.env("GITHUB_TOKEN", &token);
            cmd.env("GH_TOKEN", &token);
            eprintln!("[workspace] PTY[{}]: injected GITHUB_TOKEN for copilot mode", tab_id);
        } else {
            eprintln!("[workspace] PTY[{}]: no GitHub token found — copilot may prompt for /login", tab_id);
        }
    }
    cmd.env("TERM", "xterm-256color");
    cmd.cwd(project_dir);

    eprintln!("[workspace] PTY[{}]: spawning {} in {:?} (cli_mode={})", tab_id, shell, project_dir, cli_mode);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    drop(pair.slave);

    let reader = pair.master.try_clone_reader().map_err(|e| format!("{}", e))?;
    let writer = pair.master.take_writer().map_err(|e| format!("{}", e))?;
    let writer = Arc::new(Mutex::new(writer));

    // Auto-launch CLI after shell starts
    let writer_clone = writer.clone();
    let tab_id_launch = tab_id.to_string();
    let cli_mode_owned = cli_mode.to_string();
    let copilot_model_owned = copilot_model.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        if let Ok(mut w) = writer_clone.lock() {
            let clear = if cfg!(windows) { "cls" } else { "clear" };

            let launch_cmd = if cli_mode_owned == "copilot" {
                let model_flag = copilot_model_owned
                    .as_deref()
                    .filter(|m| !m.is_empty())
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                format!("{} && copilot{}\r\n", clear, model_flag)
            } else {
                let model_flag = model
                    .as_deref()
                    .filter(|m| !m.is_empty())
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                let fallback_flag = fallback_model
                    .as_deref()
                    .filter(|m| !m.is_empty() && Some(*m) != model.as_deref())
                    .map(|m| format!(" --fallback-model {}", m))
                    .unwrap_or_default();
                let dangerous_flag = if dangerous_mode { " --dangerously-skip-permissions" } else { "" };
                if has_memory {
                    format!("{} && claude{}{}{} \"Load Memory\"\r\n", clear, model_flag, fallback_flag, dangerous_flag)
                } else {
                    format!("{} && claude{}{}{}\r\n", clear, model_flag, fallback_flag, dangerous_flag)
                }
            };

            let _ = w.write_all(launch_cmd.as_bytes());
            let _ = w.flush();
            eprintln!("[workspace] PTY[{}]: launched {} (has_memory={}, model={:?}, fallback={:?}, cli_mode={})",
                tab_id_launch,
                if cli_mode_owned == "copilot" { "copilot" } else { "claude" },
                has_memory, model, fallback_model, cli_mode_owned);
        }
    });

    // Read PTY output in a background thread, tagged with tab_id
    let app_handle = app.clone();
    let tab_id_reader = tab_id.to_string();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        eprintln!("[workspace] PTY[{}] reader thread started", tab_id_reader);
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    eprintln!("[workspace] PTY[{}] reader: EOF", tab_id_reader);
                    break;
                }
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_handle.emit("pty-output", PtyOutputEvent {
                        tab_id: tab_id_reader.clone(),
                        data,
                    });
                }
                Err(e) => {
                    eprintln!("[workspace] PTY[{}] reader error: {}", tab_id_reader, e);
                    break;
                }
            }
        }
        eprintln!("[workspace] PTY[{}] reader thread exiting", tab_id_reader);
    });

    Ok(PtyState {
        writer,
        _child: Mutex::new(child),
        _master: Mutex::new(pair.master),
    })
}

pub fn write_to_pty(state: &PtyState, data: &str) -> Result<(), String> {
    let mut writer = state.writer.lock().map_err(|e| format!("{}", e))?;

    // Use bracketed paste mode for multi-line or large content
    // This tells the terminal app (e.g. Claude Code) to treat it as pasted text
    if data.contains('\n') || data.len() > 200 {
        writer
            .write_all(b"\x1b[200~")
            .map_err(|e| format!("PTY write error: {}", e))?;
        writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("PTY write error: {}", e))?;
        writer
            .write_all(b"\x1b[201~")
            .map_err(|e| format!("PTY write error: {}", e))?;
    } else {
        writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("PTY write error: {}", e))?;
    }

    writer.flush().map_err(|e| format!("PTY flush error: {}", e))?;
    Ok(())
}

pub fn resize_pty(state: &PtyState, rows: u16, cols: u16) -> Result<(), String> {
    if let Ok(master) = state._master.lock() {
        let _ = master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
    Ok(())
}

/// Read the GitHub OAuth token stored by copilot-api from its known locations.
fn read_github_token() -> Option<String> {
    let home = dirs::home_dir()?;

    let mut candidates: Vec<std::path::PathBuf> = vec![
        home.join(".local").join("share").join("copilot-api").join("github_token"),
        home.join(".copilot-api").join("github_token"),
    ];
    if let Some(local) = dirs::data_local_dir() {
        candidates.push(local.join("copilot-api").join("github_token"));
    }
    if let Some(roaming) = dirs::config_dir() {
        candidates.push(roaming.join("copilot-api").join("github_token"));
    }

    for path in &candidates {
        if path.exists() {
            if let Ok(token) = std::fs::read_to_string(path) {
                let token = token.trim().to_string();
                if !token.is_empty() {
                    eprintln!("[pty] read GitHub token from {:?}", path);
                    return Some(token);
                }
            }
        }
    }
    None
}
