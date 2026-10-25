//! Runs an external tool and returns what it printed, with a time limit.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use super::{no_console_window, no_window};
use crate::core::patches::py_list;

/// How often a running tool is checked for its end.
const POLL: Duration = Duration::from_millis(20);

/// Where the tool's error output goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stderr {
    Inherit,
    Null,
}

/// Whether the tool gets a console window of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    /// Never.
    Hidden,
    /// Only when this program has a console to share.
    LikeConsole,
}

/// One tool to run: no input, its output captured.
pub struct ToolCall<'a> {
    pub program: OsString,
    pub args: Vec<OsString>,
    /// The whole environment of the tool; `None` leaves it this program's.
    pub env: Option<&'a IndexMap<String, String>>,
    pub cwd: Option<&'a Path>,
    /// `None` waits for as long as the tool runs.
    pub limit: Option<Duration>,
    pub stderr: Stderr,
    pub window: Window,
}

/// What a tool that ended printed.
#[derive(Debug)]
pub struct ToolOutput {
    /// `None` when a signal ended it.
    pub code: Option<i32>,
    /// UTF-8, lossy.
    pub stdout: String,
}

#[derive(Debug)]
pub enum ToolError {
    Spawn(io::Error),
    TimedOut {
        command: Vec<String>,
        limit: Duration,
    },
    Wait(io::Error),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) | Self::Wait(error) => error.fmt(f),
            // subprocess.TimeoutExpired's text (run.py:132).
            Self::TimedOut { command, limit } => write!(
                f,
                "Command '{}' timed out after {} seconds",
                py_list(command),
                limit.as_secs()
            ),
        }
    }
}

impl std::error::Error for ToolError {}

/// Runs `call` and waits for it; a tool still running at its limit is stopped.
pub fn run_tool(call: &ToolCall) -> Result<ToolOutput, ToolError> {
    let mut cmd = Command::new(&call.program);
    cmd.args(&call.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    if let Some(env) = call.env {
        cmd.env_clear().envs(env);
    }
    if let Some(cwd) = call.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stderr(match call.stderr {
        Stderr::Inherit => Stdio::inherit(),
        Stderr::Null => Stdio::null(),
    });
    match call.window {
        Window::Hidden => no_window(&mut cmd),
        Window::LikeConsole => no_console_window(&mut cmd),
    }
    let mut child = cmd.spawn().map_err(ToolError::Spawn)?;
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ToolError::Wait(io::Error::other("no output")));
    };
    let reader = std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stdout.read_to_end(&mut text);
        text
    });
    let deadline = call.limit.map(|limit| Instant::now() + limit);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if deadline.is_none_or(|at| Instant::now() < at) => {
                std::thread::sleep(POLL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let command = std::iter::once(&call.program)
                    .chain(&call.args)
                    .map(|part| part.to_string_lossy().into_owned())
                    .collect();
                return Err(ToolError::TimedOut {
                    command,
                    limit: call.limit.unwrap_or_default(),
                });
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError::Wait(error));
            }
        }
    };
    let text = reader.join().unwrap_or_default();
    Ok(ToolOutput {
        code: status.code(),
        stdout: String::from_utf8_lossy(&text).into_owned(),
    })
}
