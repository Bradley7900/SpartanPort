//! The runner (Python `run.py`): starts the game under the probe and reports what it saw.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use indexmap::IndexMap;

/// What one run needs from the program around it.
pub struct RunContext<'a> {
    pub port_dir: &'a Path,
    pub env: IndexMap<String, String>,
    pub args: Vec<OsString>,
    pub out: &'a mut dyn Write,
}

/// Runs the runner and returns its exit code.
pub fn run(context: RunContext<'_>) -> i32 {
    let probe = if cfg!(windows) {
        "spartanport-probe.exe"
    } else {
        "spartanport-probe"
    };
    // A closed stdout is nothing to report on: the code says it.
    let _ = writeln!(
        context.out,
        "{probe} not found: build it with build.sh or use a packaged build."
    );
    1
}
