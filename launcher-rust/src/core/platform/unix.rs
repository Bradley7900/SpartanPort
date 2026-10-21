//! Calls for systems other than Windows.

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::time::Duration;

/// Time between SIGTERM and SIGKILL.
const KILL_AFTER: Duration = Duration::from_secs(3);

/// There is no Windows UI language here.
pub fn user_default_ui_language() -> Option<u32> {
    None
}

/// Puts the child in its own process group.
pub fn prepare(cmd: &mut Command) {
    cmd.process_group(0);
}

/// The game's process group.
pub struct Job {
    pgid: i32,
}

fn signal_group(pgid: i32, signal: Signal) {
    // A group that is gone (ESRCH) or not ours (EPERM) is not worth reporting; nothing to undo.
    let _ = killpg(Pid::from_raw(pgid), signal);
}

impl Job {
    pub fn attach(child: &Child) -> Job {
        Job {
            pgid: i32::try_from(child.id()).unwrap_or(0),
        }
    }

    /// SIGTERM to the group, then SIGKILL after three seconds.
    pub fn terminate(&mut self) {
        if self.pgid <= 0 {
            return;
        }
        let pgid = self.pgid;
        signal_group(pgid, Signal::SIGTERM);
        std::thread::spawn(move || {
            std::thread::sleep(KILL_AFTER);
            signal_group(pgid, Signal::SIGKILL);
        });
    }

    pub fn close(&mut self) {}
}
