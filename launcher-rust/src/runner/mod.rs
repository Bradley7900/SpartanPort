//! The runner (Python `run.py`): starts the game under the probe and reports what it saw.

pub mod guest_memory;
pub mod link_libc;
pub mod link_modules;
pub mod prepare;
pub mod window_icon;

/// The synthetic probe inputs the runner's tests build (`tests/test_probe.py::package` and more).
#[cfg(test)]
#[path = "../../tests/common/packages.rs"]
mod packages;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::paths::Paths;
use crate::core::platform;

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

/// Instruction sets `spartanport-probe --cpu-features` reports missing (empty when complete).
pub fn parse_cpu_features(output: &str) -> Vec<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("CPU: missing "))
        .map(|names| {
            names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `SPARTAN_ISA_REWRITE=1` forces the ISA rewrite table, `0` disables it; otherwise the CPU decides.
pub fn isa_rewrite_wanted(missing: &[String], env: &dyn Fn(&str) -> Option<String>) -> bool {
    match env("SPARTAN_ISA_REWRITE").as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => !missing.is_empty(),
    }
}

/// The command that starts the game on Linux when GameMode is chosen in the launcher (empty on
/// Windows, without the choice, or when `gamemoderun` is not on `PATH`).
pub fn launch_prefix(env: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    if cfg!(windows) || env("SPARTAN_GAMEMODE").as_deref() != Some("1") {
        return Vec::new();
    }
    match env("PATH").and_then(|path| which("gamemoderun", &path)) {
        Some(found) => vec![found.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

/// The first executable file called `name` in the directories of `path`.
fn which(name: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// The probe: `SPARTAN_PROBE` when set (taken as is, like `run.py`), else `bin/` or `out/` of the port.
pub fn find_executable(paths: &Paths, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env("SPARTAN_PROBE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| paths.find_tool("spartanport-probe"))
}

/// Waits up to `limit` for process `pid` to end (the previous game, for `--after`). `true` when it
/// ended or never existed.
pub fn wait_for_pid(pid: u32, limit: Duration) -> bool {
    platform::wait_for_process(pid, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn parse_cpu_features_reads_the_missing_line() {
        assert_eq!(
            parse_cpu_features("CPU: missing MOVBE, BMI1, LZCNT\n"),
            ["MOVBE", "BMI1", "LZCNT"]
        );
        assert!(parse_cpu_features("CPU: complete\n").is_empty());
    }

    #[test]
    fn rewrite_wanted_follows_cpu_by_default() {
        assert!(isa_rewrite_wanted(&["MOVBE".to_string()], &env_of(&[])));
        assert!(!isa_rewrite_wanted(&[], &env_of(&[])));
    }

    #[test]
    fn rewrite_forced() {
        assert!(isa_rewrite_wanted(
            &[],
            &env_of(&[("SPARTAN_ISA_REWRITE", "1")])
        ));
    }

    #[test]
    fn rewrite_disabled() {
        let missing = ["MOVBE".to_string(), "BMI1".to_string()];
        assert!(!isa_rewrite_wanted(
            &missing,
            &env_of(&[("SPARTAN_ISA_REWRITE", "0")])
        ));
    }

    #[test]
    fn launch_prefix_is_empty_without_gamemode() {
        assert!(launch_prefix(&env_of(&[])).is_empty());
    }

    #[test]
    fn find_executable_prefers_the_probe_variable() {
        let dir = tempfile::tempdir().unwrap();
        let probe = dir.path().join("my-probe");
        std::fs::write(&probe, b"").unwrap();
        let paths = Paths::from_env(dir.path(), &|_| None);
        let env = env_of(&[("SPARTAN_PROBE", probe.to_str().unwrap_or(""))]);
        assert_eq!(find_executable(&paths, &env), Some(probe));
        assert_eq!(find_executable(&paths, &env_of(&[])), None);
    }
}
