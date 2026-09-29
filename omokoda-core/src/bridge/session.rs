use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Options for spawning a child omokoda-cli session.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// Path to the omokoda CLI binary
    pub binary: String,
    /// Arguments to pass
    pub args: Vec<String>,
    /// Optional transcript file path for NDJSON logging
    pub transcript_path: Option<String>,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            binary: "omokoda-cli".to_string(),
            args: vec![],
            transcript_path: None,
        }
    }
}

/// A permission request detected in the NDJSON stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub tool_use_id: String,
}

/// Activity emitted by the session as it reads the child's NDJSON output.
#[derive(Debug, Clone)]
pub enum SessionActivity {
    ToolStart {
        tool: String,
        input: serde_json::Value,
    },
    Text(String),
    Result(serde_json::Value),
    Error(String),
    PermissionNeeded(PermissionRequest),
}

/// Live handle to a spawned child session.
pub struct SessionHandle {
    child: Child,
    stdin: ChildStdin,
    /// Rolling ring buffer of last 10 stderr lines
    stderr_ring: Arc<Mutex<VecDeque<String>>>,
}

impl SessionHandle {
    /// Send raw data to child's stdin.
    pub fn write_stdin(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.stdin.write_all(data)?;
        self.stdin.flush()
    }

    /// Send a control message as NDJSON.
    pub fn send_control(&mut self, msg: &serde_json::Value) -> std::io::Result<()> {
        let line = serde_json::to_string(msg).unwrap_or_default();
        self.write_stdin(format!("{}\n", line).as_bytes())
    }

    /// Refresh auth tokens via stdin control message.
    pub fn refresh_tokens(
        &mut self,
        env_vars: std::collections::HashMap<String, String>,
    ) -> std::io::Result<()> {
        let msg = serde_json::json!({
            "type": "update_environment_variables",
            "env": env_vars,
        });
        self.send_control(&msg)
    }

    /// Graceful shutdown — sends SIGTERM.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    /// Immediate shutdown — sends SIGKILL.
    pub fn force_kill(&mut self) {
        let pid = self.child.id();
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }

    /// Last N stderr lines captured from the child.
    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr_ring.lock().unwrap().iter().cloned().collect()
    }
}

/// Spawns child omokoda-cli processes and streams their NDJSON output.
pub struct SessionSpawner;

impl SessionSpawner {
    /// Spawn a new child session. Returns a handle for control and an iterator of activities.
    pub fn spawn(opts: SpawnOptions) -> std::io::Result<(SessionHandle, NdjsonStream)> {
        let mut cmd = Command::new(&opts.binary);
        cmd.args(&opts.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().expect("stdin configured");
        let stdout = child.stdout.take().expect("stdout configured");

        let stderr_ring: Arc<Mutex<VecDeque<String>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(10)));
        let ring_clone = stderr_ring.clone();

        // Spawn thread to drain stderr into ring buffer
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(|l| l.ok()) {
                    let mut ring = ring_clone.lock().unwrap();
                    if ring.len() >= 10 {
                        ring.pop_front();
                    }
                    ring.push_back(line);
                }
            });
        }

        let handle = SessionHandle {
            child,
            stdin,
            stderr_ring,
        };
        let stream = NdjsonStream::new(stdout, opts.transcript_path);
        Ok((handle, stream))
    }
}

/// Iterator that reads NDJSON lines from child stdout and emits SessionActivity.
pub struct NdjsonStream {
    reader: BufReader<ChildStdout>,
    transcript: Option<std::fs::File>,
}

impl NdjsonStream {
    fn new(stdout: ChildStdout, transcript_path: Option<String>) -> Self {
        let transcript = transcript_path.and_then(|p| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
        Self {
            reader: BufReader::new(stdout),
            transcript,
        }
    }

    fn log_line(&mut self, line: &str) {
        if let Some(f) = &mut self.transcript {
            let _ = writeln!(f, "{}", line);
        }
    }
}

impl Iterator for NdjsonStream {
    type Item = SessionActivity;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => {
                let trimmed = line.trim_end();
                self.log_line(trimmed);

                let v: serde_json::Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(_) => return Some(SessionActivity::Text(trimmed.to_string())),
                };

                let kind = v["type"].as_str().unwrap_or("");

                match kind {
                    "tool_use" => Some(SessionActivity::ToolStart {
                        tool: v["name"].as_str().unwrap_or("").to_string(),
                        input: v["input"].clone(),
                    }),
                    "text" => Some(SessionActivity::Text(
                        v["text"].as_str().unwrap_or("").to_string(),
                    )),
                    "result" => Some(SessionActivity::Result(v["output"].clone())),
                    "error" => Some(SessionActivity::Error(
                        v["message"].as_str().unwrap_or("unknown error").to_string(),
                    )),
                    "control_request" => {
                        // Permission gate: child is asking if it may use a tool
                        if let Ok(req) =
                            serde_json::from_value::<PermissionRequest>(v["request"].clone())
                        {
                            Some(SessionActivity::PermissionNeeded(req))
                        } else {
                            Some(SessionActivity::Error(
                                "malformed control_request".to_string(),
                            ))
                        }
                    }
                    _ => Some(SessionActivity::Text(trimmed.to_string())),
                }
            }
        }
    }
}
