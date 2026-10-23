//! A stand-in for `spartanport-probe` used by the runner tests (`tests/runner.rs`).
//!
//! - `--cpu-features`: prints `CPU: missing <FAKE_PROBE_MISSING>` when that list is set, and
//!   exits with `FAKE_PROBE_CPU_EXIT` (default 0).
//! - `--gamepad`: prints `FAKE_PROBE_GAMEPAD`; hangs when it is `hang`.
//! - `--displays`: prints nothing.
//! - Any other run (the game): prints `fake-probe argv:`, its argv one per line and
//!   `fake-probe argv end`; then `NAME=value` (or `NAME is unset`) for each variable named in
//!   `FAKE_PROBE_PRINT` (comma-separated); with `FAKE_PROBE_SHOW=<file>`, the text of that file
//!   under its `--app0` folder (`fake-probe show <file>: <text>`, or `missing`). With
//!   `FAKE_PROBE_RESTART=1` and `SPARTAN_RESTART_COMMAND` set it prints
//!   `Runtime: restarting through run.py` and starts that command as the probe does (Windows: the
//!   command line, with this program's output handles, then exit 0; Linux: `sh -c` in place of
//!   itself), with `FAKE_PROBE_RESTART=0` so the new launch does not restart again. Otherwise it
//!   sleeps `FAKE_PROBE_SLEEP_MS` (default 0) and exits with `FAKE_PROBE_EXIT` (default 0).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::Path;
use std::time::Duration;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn code(name: &str) -> i32 {
    var(name).parse().unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut out = std::io::stdout();
    match args.get(1).map(String::as_str) {
        Some("--cpu-features") => {
            let missing = var("FAKE_PROBE_MISSING");
            if !missing.is_empty() {
                writeln!(out, "CPU: missing {missing}").unwrap();
            }
            out.flush().unwrap();
            std::process::exit(code("FAKE_PROBE_CPU_EXIT"));
        }
        Some("--gamepad") => {
            let pad = var("FAKE_PROBE_GAMEPAD");
            if pad == "hang" {
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
            writeln!(out, "{pad}").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        Some("--displays") => std::process::exit(0),
        _ => {}
    }
    writeln!(out, "fake-probe argv:").unwrap();
    for arg in &args {
        writeln!(out, "{arg}").unwrap();
    }
    writeln!(out, "fake-probe argv end").unwrap();
    for name in var("FAKE_PROBE_PRINT").split(',').filter(|n| !n.is_empty()) {
        match std::env::var(name) {
            Ok(value) => writeln!(out, "{name}={value}").unwrap(),
            Err(_) => writeln!(out, "{name} is unset").unwrap(),
        }
    }
    let show = var("FAKE_PROBE_SHOW");
    if !show.is_empty() {
        let app0 = args
            .iter()
            .position(|arg| arg == "--app0")
            .and_then(|at| args.get(at + 1))
            .unwrap();
        let text = std::fs::read_to_string(Path::new(app0).join(&show))
            .unwrap_or_else(|_| "missing".to_string());
        writeln!(out, "fake-probe show {show}: {text}").unwrap();
    }
    let command = var("SPARTAN_RESTART_COMMAND");
    if var("FAKE_PROBE_RESTART") == "1" && !command.is_empty() {
        writeln!(out, "Runtime: restarting through run.py").unwrap();
        out.flush().unwrap();
        restart(&command);
    }
    out.flush().unwrap();
    std::thread::sleep(Duration::from_millis(
        var("FAKE_PROBE_SLEEP_MS").parse().unwrap_or(0),
    ));
    std::process::exit(code("FAKE_PROBE_EXIT"));
}

/// `CreateProcess(NULL, command)` with the inherited output handles, then exit 0, as the probe's
/// `runtime_restart`: the program is the first word of the line (quoted or not), the rest is
/// passed as written.
#[cfg(windows)]
fn restart(command: &str) {
    use std::os::windows::process::CommandExt;
    let (program, rest) = match command.strip_prefix('"') {
        Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
        None => command.split_once([' ', '\t']).unwrap_or((command, "")),
    };
    std::process::Command::new(program)
        .raw_arg(rest.trim_start())
        .env("FAKE_PROBE_RESTART", "0")
        .spawn()
        .unwrap();
    std::process::exit(0);
}

/// `sh -c command` in place of this process, as the probe's `runtime_restart`.
#[cfg(not(windows))]
fn restart(command: &str) {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .env("FAKE_PROBE_RESTART", "0")
        .exec();
    eprintln!("fake-probe: exec: {error}");
    std::process::exit(1);
}
