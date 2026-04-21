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

pub fn spawn_pty(app: &AppHandle, tab_id: &str, project_dir: &str, rewriter_port: u16, has_memory: bool, model: Option<String>, fallback_model: Option<String>, dangerous_mode: bool) -> Result<PtyState, String> {
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
    cmd.env("ANTHROPIC_BASE_URL", format!("http://localhost:{}", rewriter_port));
    cmd.env("DISABLE_NON_ESSENTIAL_MODEL_CALLS", "1");
    cmd.env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
    cmd.env("TERM", "xterm-256color");
    cmd.cwd(project_dir);

    eprintln!("[workspace] PTY[{}]: spawning {} in {:?}", tab_id, shell, project_dir);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    drop(pair.slave);

    let reader = pair.master.try_clone_reader().map_err(|e| format!("{}", e))?;
    let writer = pair.master.take_writer().map_err(|e| format!("{}", e))?;
    let writer = Arc::new(Mutex::new(writer));

    // Auto-launch claude after shell starts
    let writer_clone = writer.clone();
    let tab_id_launch = tab_id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        if let Ok(mut w) = writer_clone.lock() {
            let clear = if cfg!(windows) { "cls" } else { "clear" };
            // Build model flag if specified
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
                let _ = w.write_all(format!("{} && claude{}{}{} \"Load Memory\"\r\n", clear, model_flag, fallback_flag, dangerous_flag).as_bytes());
            } else {
                let _ = w.write_all(format!("{} && claude{}{}{}\r\n", clear, model_flag, fallback_flag, dangerous_flag).as_bytes());
            }
            let _ = w.flush();
            eprintln!("[workspace] PTY[{}]: launched claude (has_memory={}, model={:?}, fallback={:?})", tab_id_launch, has_memory, model, fallback_model);
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
