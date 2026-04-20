use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::logging::{self, LogBuffer};

const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Check if a port is in use by attempting a TCP connection to it.
fn port_in_use(port: u16) -> bool {
    if TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(200),
    )
    .is_ok()
    {
        return true;
    }
    if TcpStream::connect_timeout(
        &format!("[::1]:{}", port).parse().unwrap(),
        Duration::from_millis(200),
    )
    .is_ok()
    {
        return true;
    }
    false
}

/// Find an available port, starting from `preferred` and incrementing by 1
fn find_available_port(preferred: u16) -> u16 {
    for port in preferred..preferred + 100 {
        if !port_in_use(port) {
            return port;
        }
    }
    preferred
}

/// Kill any process listening on a given port (orphan cleanup).
/// Uses netstat + taskkill on Windows.
fn kill_process_on_port(port: u16, app: Option<&AppHandle>, logs: Option<&LogBuffer>) {
    let output = Command::new("cmd")
        .args(["/c", &format!("netstat -ano | findstr :{} | findstr LISTENING", port)])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // netstat line format: "  TCP    0.0.0.0:4141    0.0.0.0:0    LISTENING    12345"
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid > 0 {
                        let msg = format!("Killing orphan process PID {} on port {}", pid, port);
                        if let (Some(app), Some(logs)) = (app, logs) {
                            logging::emit_log(app, logs, "proxy", "warn", &msg);
                        } else {
                            eprintln!("[workspace] {}", msg);
                        }
                        let _ = Command::new("taskkill")
                            .args(["/T", "/F", "/PID", &pid.to_string()])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output();
                    }
                }
            }
        }
    }
}

/// Kill an entire process tree on Windows using taskkill /T /F
fn tree_kill(child: &mut Child) {
    let pid = child.id();
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    // Reap the child so we don't leave a zombie handle
    let _ = child.wait();
}

/// Spawn a thread that reads lines from a process stream and emits log entries
fn pipe_output_to_logs(
    reader: impl std::io::Read + Send + 'static,
    app: AppHandle,
    logs: Arc<LogBuffer>,
    source: String,
) {
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            match line {
                Ok(line) if !line.is_empty() => {
                    logging::emit_log(&app, &logs, &source, "info", &line);
                }
                Err(_) => break,
                _ => {}
            }
        }
    });
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProcessStatus {
    pub name: String,
    pub running: bool,
    pub port: Option<u16>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PortAssignments {
    pub copilot_proxy: u16,
    pub model_rewriter: u16,
}

pub struct ManagedProcesses {
    pub copilot_proxy: Arc<Mutex<Option<Child>>>,
    pub model_rewriter_running: Arc<Mutex<bool>>,
    pub ports: Arc<Mutex<PortAssignments>>,
}

impl ManagedProcesses {
    pub fn new() -> Self {
        Self {
            copilot_proxy: Arc::new(Mutex::new(None)),
            model_rewriter_running: Arc::new(Mutex::new(false)),
            ports: Arc::new(Mutex::new(PortAssignments {
                copilot_proxy: 4141,
                model_rewriter: 4142,
            })),
        }
    }

    /// Kill orphan processes on default proxy ports (call before starting)
    pub fn cleanup_orphans(&self, app: &AppHandle, logs: &LogBuffer) {
        let ports = self.ports.lock().unwrap();
        let cp = ports.copilot_proxy;
        let rw = ports.model_rewriter;
        drop(ports);

        if port_in_use(cp) {
            kill_process_on_port(cp, Some(app), Some(logs));
            // Give OS time to release the port
            std::thread::sleep(Duration::from_millis(500));
        }
        if port_in_use(rw) {
            kill_process_on_port(rw, Some(app), Some(logs));
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    pub fn start_copilot_proxy(&self, app: &AppHandle, logs: &LogBuffer) -> Result<u16, String> {
        let port = find_available_port(4141);
        logging::emit_log(app, logs, "proxy", "info", &format!("Starting copilot-api on port {}", port));

        let mut child = Command::new("cmd")
            .args([
                "/c",
                "npx",
                "copilot-api@latest",
                "start",
                "-p",
                &port.to_string(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to start copilot proxy: {}", e))?;

        // Pipe stdout/stderr into log system
        let logs_arc = Arc::new(logs.clone_inner());
        if let Some(stdout) = child.stdout.take() {
            pipe_output_to_logs(stdout, app.clone(), logs_arc.clone(), "proxy".into());
        }
        if let Some(stderr) = child.stderr.take() {
            pipe_output_to_logs(stderr, app.clone(), logs_arc, "proxy".into());
        }

        logging::emit_log(
            app,
            logs,
            "proxy",
            "info",
            &format!("copilot-api started (PID {})", child.id()),
        );

        *self.copilot_proxy.lock().unwrap() = Some(child);
        self.ports.lock().unwrap().copilot_proxy = port;

        let _ = app.emit(
            "process-status",
            ProcessStatus {
                name: "copilot-proxy".into(),
                running: true,
                port: Some(port),
            },
        );

        Ok(port)
    }

    pub fn start_model_rewriter(
        &self,
        app: &AppHandle,
        logs: &LogBuffer,
        upstream_port: u16,
    ) -> Result<u16, String> {
        let port = find_available_port(upstream_port + 1);

        logging::emit_log(
            app,
            logs,
            "proxy",
            "info",
            &format!(
                "Starting model-rewrite-proxy on port {} (upstream={})",
                port, upstream_port
            ),
        );

        // Start the built-in Rust proxy
        crate::model_rewriter::start(upstream_port, port);

        *self.model_rewriter_running.lock().unwrap() = true;
        self.ports.lock().unwrap().model_rewriter = port;

        logging::emit_log(
            app,
            logs,
            "proxy",
            "info",
            &format!("model-rewrite-proxy started (built-in, port {})", port),
        );

        let _ = app.emit(
            "process-status",
            ProcessStatus {
                name: "model-rewriter".into(),
                running: true,
                port: Some(port),
            },
        );

        Ok(port)
    }

    pub fn get_rewriter_port(&self) -> u16 {
        self.ports.lock().unwrap().model_rewriter
    }

    /// Stop all managed processes using tree-kill (kills entire process trees)
    pub fn stop_all(&self) {
        if let Ok(mut child) = self.copilot_proxy.lock() {
            if let Some(ref mut c) = *child {
                tree_kill(c);
            }
            *child = None;
        }
        // Model rewriter is an in-process tokio task — it dies with the app
        *self.model_rewriter_running.lock().unwrap() = false;
    }

    pub fn get_statuses(&self) -> Vec<ProcessStatus> {
        let ports = self.ports.lock().unwrap();

        // Use try_wait to detect crashed processes
        let copilot_running = self
            .copilot_proxy
            .lock()
            .map(|mut c| match c.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(None) => true,  // still running
                    Ok(Some(_)) => false, // exited
                    Err(_) => false,
                },
                None => false,
            })
            .unwrap_or(false);

        let rewriter_running = *self.model_rewriter_running.lock().unwrap_or_else(|e| e.into_inner());

        vec![
            ProcessStatus {
                name: "copilot-proxy".into(),
                running: copilot_running,
                port: Some(ports.copilot_proxy),
            },
            ProcessStatus {
                name: "model-rewriter".into(),
                running: rewriter_running,
                port: Some(ports.model_rewriter),
            },
        ]
    }
}
