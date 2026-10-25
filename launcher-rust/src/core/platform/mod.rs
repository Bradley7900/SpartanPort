//! The few calls that need the operating system.

pub mod ipc;
pub mod tool;
use std::path::PathBuf;

use crate::core::env::Lookup;

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unix::{
    attach_parent_console, catch_interrupt, close_std_streams, create_junction, interrupted,
    no_console_window, no_window, prepare, show_error, steam_path_from_registry,
    user_default_ui_language, wait_for_process, Job,
};
#[cfg(windows)]
pub use windows::{
    attach_parent_console, catch_interrupt, close_std_streams, create_junction, has_console,
    interrupted, no_console_window, no_window, prepare, show_error, steam_path_from_registry,
    user_default_ui_language, wait_for_process, Job,
};

/// The user's home folder as `env` names it (`USERPROFILE` on Windows, `HOME` elsewhere), like
/// Python's `Path.home()`. An empty value is no home.
pub fn home(env: Lookup) -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_userprofile_or_home_never_empty() {
        let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let other = if cfg!(windows) { "HOME" } else { "USERPROFILE" };
        let set = |k: &str| (k == key).then(|| "C:/u".to_string());
        assert_eq!(home(&set), Some(PathBuf::from("C:/u")));
        // Only the variable of this system counts.
        assert_eq!(home(&|k| (k == other).then(|| "x".to_string())), None);
        assert_eq!(home(&|k| (k == key).then(String::new)), None);
        assert_eq!(home(&|_| None), None);
    }
}
