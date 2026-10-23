//! The few calls that need the operating system.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unix::{
    attach_parent_console, close_std_streams, create_junction, no_window, prepare,
    user_default_ui_language, wait_for_process, Job,
};
#[cfg(windows)]
pub use windows::{
    attach_parent_console, close_std_streams, create_junction, no_window, pipe_bytes_available,
    prepare, set_pipe_nowait, user_default_ui_language, wait_for_process, Job,
};
