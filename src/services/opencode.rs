use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use serde_json::Value;

use crate::services::claude::{CancelToken, StreamMessage};

/// Cached path to the opencode binary.
static OPENCODE_PATH: OnceLock<Option<String>> = OnceLock::new();

fn resolve_opencode_path() -> Option<String> {
    // Try direct `which opencode` first
    if let Ok(output) = Command::new("which").arg("opencode").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // Fallback: use login shell to resolve PATH
    if let Ok(output) = Command::new("bash")
        .args(["-lc", "which opencode"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

fn get_opencode_path() -> Option<&'static str> {
    OPENCODE_PATH
        .get_or_init(|| resolve_opencode_path())
        .as_deref()
}

#[derive(Debug, Clone)]
pub struct OpenCodeServer {
    pub url: String,
    pub pid: u32,
}

/// Start an OpenCode headless server bound to 127.0.0.1 with a random port.
/// Returns the server URL and PID.
pub fn start_server(working_dir: &str) -> Result<OpenCodeServer, String> {
    let bin = get_opencode_path().ok_or_else(|| "opencode not found on PATH".to_string())?;

    let mut child = Command::new(bin)
        .args([
            "serve",
            "--port",
            "0",
            "--hostname",
            "127.0.0.1",
            "--log-level",
            "INFO",
        ])
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start opencode serve: {e}"))?;

    let pid = child.id();

    // Read stdout/stderr until we see the listening URL.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture opencode serve stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture opencode serve stderr".to_string())?;

    let (tx, rx) = std::sync::mpsc::channel::<String>();

    std::thread::spawn({
        let tx = tx.clone();
        move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let _ = tx.send(line);
            }
        }
    });

    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            let _ = tx.send(line);
        }
    });

    let mut url: Option<String> = None;

    // Wait up to ~5 seconds for the URL.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(200)) {
            // Typical: "opencode server listening on http://127.0.0.1:4096"
            if let Some(idx) = line.find("http://") {
                url = Some(line[idx..].trim().to_string());
                break;
            }
        }
    }

    // IMPORTANT: do NOT wait on the child; dropping Child does not kill the process.
    // We intentionally leak the child handle so the process continues running.
    std::mem::forget(child);

    let url = url.ok_or_else(|| "Failed to detect opencode server URL from logs".to_string())?;
    Ok(OpenCodeServer { url, pid })
}

/// Stop an OpenCode headless server by PID.
pub fn stop_server(pid: u32) {
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
}

/// Execute an OpenCode message against a running server (attach mode) and stream JSON events.
///
/// We map OpenCode JSON events into the existing StreamMessage enum used by the Telegram bridge.
pub fn execute_run_streaming(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    server_url: &str,
    sender: Sender<StreamMessage>,
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    let bin = get_opencode_path().ok_or_else(|| "opencode not found on PATH".to_string())?;

    let mut args: Vec<String> = vec![
        "run".to_string(),
        prompt.to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--attach".to_string(),
        server_url.to_string(),
        "--dir".to_string(),
        working_dir.to_string(),
    ];

    if let Some(sid) = session_id {
        args.push("--session".to_string());
        args.push(sid.to_string());
    }

    let mut child = Command::new(bin)
        .args(&args)
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start opencode run: {e}"))?;

    // Store PID for /stop
    if let Some(ref token) = cancel_token {
        *token.child_pid.lock().unwrap() = Some(child.id());
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture opencode stdout".to_string())?;
    let reader = BufReader::new(stdout);

    let mut last_session_id: Option<String> = None;

    for line in reader.lines() {
        // Cancellation
        if let Some(ref token) = cancel_token {
            if token.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
        }

        let line = line.map_err(|e| format!("Failed to read opencode output: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<Value>(&line) {
            if let Some(sid) = json.get("sessionID").and_then(|v| v.as_str()) {
                if last_session_id.as_deref() != Some(sid) {
                    last_session_id = Some(sid.to_string());
                    let _ = sender.send(StreamMessage::Init {
                        session_id: sid.to_string(),
                    });
                }
            }

            // Map common event types
            if let Some(t) = json.get("type").and_then(|v| v.as_str()) {
                match t {
                    "text" => {
                        if let Some(text) = json
                            .get("part")
                            .and_then(|p| p.get("text"))
                            .and_then(|v| v.as_str())
                        {
                            if !text.is_empty() {
                                let _ = sender.send(StreamMessage::Text {
                                    content: text.to_string(),
                                });
                            }
                        }
                    }
                    "tool_use" => {
                        let name = json
                            .get("part")
                            .and_then(|p| p.get("tool"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool")
                            .to_string();
                        let input = json
                            .get("part")
                            .and_then(|p| p.get("state"))
                            .and_then(|s| s.get("input"))
                            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                            .unwrap_or_default();
                        let _ = sender.send(StreamMessage::ToolUse { name, input });

                        // If there is an output, emit ToolResult too
                        if let Some(output) = json
                            .get("part")
                            .and_then(|p| p.get("state"))
                            .and_then(|s| s.get("output"))
                        {
                            let out_str = if output.is_string() {
                                output.as_str().unwrap_or("").to_string()
                            } else {
                                serde_json::to_string_pretty(output).unwrap_or_default()
                            };
                            if !out_str.is_empty() {
                                let _ = sender.send(StreamMessage::ToolResult {
                                    content: out_str,
                                    is_error: false,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Also read stderr (best-effort) if process fails.
    let status = child
        .wait()
        .map_err(|e| format!("Failed waiting opencode: {e}"))?;

    if !status.success() {
        let mut err = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut err);
        }
        let _ = sender.send(StreamMessage::Error {
            message: if err.is_empty() {
                format!("opencode exited with code {:?}", status.code())
            } else {
                err
            },
        });
        return Err(format!("opencode exited with code {:?}", status.code()));
    }

    let _ = sender.send(StreamMessage::Done {
        result: String::new(),
        session_id: last_session_id,
    });

    Ok(())
}
