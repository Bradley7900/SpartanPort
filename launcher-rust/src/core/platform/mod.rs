//! The few calls that need the operating system.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unix::user_default_ui_language;
#[cfg(windows)]
pub use windows::user_default_ui_language;
