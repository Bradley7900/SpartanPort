//! The few calls that need the operating system.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unix::{close_std_streams, prepare, user_default_ui_language, Job};
#[cfg(windows)]
pub use windows::{close_std_streams, prepare, user_default_ui_language, Job};
