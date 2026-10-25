//! What the runner asks of the system around it: tools to run, the game to start and wait for,
//! Ctrl+C, this program's identity and the installed memory. [`SystemHost`] is the real one; the
//! tests give the runner their own.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{discord, exit_code, guest_memory, this_exe, RunContext};
use crate::core::platform::tool::{run_tool, Stderr, ToolCall, ToolError, ToolOutput, Window};
use crate::core::platform::{self, no_console_window, no_window};

pub trait Host {
    /// Runs a tool to its end (`--cpu-features`, `--gamepad`).
    fn tool(&self, call: &ToolCall) -> Result<ToolOutput, ToolError>;
    /// Starts the game and waits for it, with the Discord presence while it runs: its exit code;
    /// `None` once Ctrl+C came first. `log` gets the presence's lines.
    fn play(&self, call: &ToolCall, log: &mut dyn FnMut(String)) -> io::Result<Option<i32>>;
    /// Waits up to `limit` for process `pid` to end; `true` when it ended or never existed.
    fn wait_for_pid(&self, pid: u32, limit: Duration) -> bool;
    /// Whether Ctrl+C came.
    fn interrupted(&self) -> bool;
    /// This program's executable and process id.
    fn this_process(&self) -> (PathBuf, u32);
    /// The installed RAM in MiB, or `None` when the system does not say.
    fn physical_memory_mb(&self) -> Option<u64>;
}

/// The real system.
pub struct SystemHost;

impl Host for SystemHost {
    fn tool(&self, call: &ToolCall) -> Result<ToolOutput, ToolError> {
        run_tool(call)
    }

    fn play(&self, call: &ToolCall, log: &mut dyn FnMut(String)) -> io::Result<Option<i32>> {
        let mut command = Command::new(&call.program);
        command.args(&call.args).stdin(Stdio::null());
        if let Some(env) = call.env {
            command.env_clear().envs(env);
        }
        if let Some(cwd) = call.cwd {
            command.current_dir(cwd);
        }
        if call.stderr == Stderr::Null {
            command.stderr(Stdio::null());
        }
        match call.window {
            Window::Hidden => no_window(&mut command),
            Window::LikeConsole => no_console_window(&mut command),
        }
        let mut child = command.spawn().map_err(|error| {
            let program = call.program.to_string_lossy();
            io::Error::new(error.kind(), format!("{program}: {error}"))
        })?;
        let lookup = |key: &str| call.env.and_then(|env| env.get(key).cloned());
        discord::while_running(child.id(), &lookup, log, || loop {
            if let Some(status) = child.try_wait()? {
                return Ok(Some(exit_code(status)));
            }
            if platform::interrupted() {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(50));
        })
    }

    fn wait_for_pid(&self, pid: u32, limit: Duration) -> bool {
        platform::wait_for_process(pid, limit)
    }

    fn interrupted(&self) -> bool {
        platform::interrupted()
    }

    fn this_process(&self) -> (PathBuf, u32) {
        (this_exe(), std::process::id())
    }

    fn physical_memory_mb(&self) -> Option<u64> {
        guest_memory::physical_memory_mb()
    }
}

/// [`super::run`] on `host`: the whole run, and the code after Ctrl+C.
pub fn run_with(context: RunContext<'_>, host: &dyn Host) -> i32 {
    let RunContext {
        port_dir,
        env,
        args,
        out,
    } = context;
    let mut runner = super::Runner::new(out, env, host);
    let code = runner.start(port_dir, args);
    if host.interrupted() {
        super::INTERRUPTED
    } else {
        code
    }
}
