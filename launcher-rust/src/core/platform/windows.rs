//! Windows calls.

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
    STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, CREATE_NO_WINDOW, PROCESS_SYNCHRONIZE,
};

/// The user's UI language id (LANGID), or `None` when Windows reports 0.
pub fn user_default_ui_language() -> Option<u32> {
    // SAFETY: the function takes no arguments and no pointers, and always succeeds.
    let id = unsafe { GetUserDefaultUILanguage() };
    (id != 0).then_some(u32::from(id))
}

/// Gives a console program no console window (the launcher, a windowed program, has none to share).
pub fn no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// Hides the console window of the child.
pub fn prepare(cmd: &mut Command) {
    no_window(cmd);
}

/// Started from a terminal, a windowed program has no console: this attaches to the terminal's, so
/// `--version` and start-up errors print there. Nothing changes when the output already goes
/// somewhere (a pipe or a file) or when there is no terminal (a double click).
pub fn attach_parent_console() {
    // SAFETY: GetStdHandle takes a plain id and returns a handle value or null/invalid.
    let out = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if !out.is_null() && out != INVALID_HANDLE_VALUE {
        return;
    }
    // SAFETY: takes a plain process id; without a parent console it fails and changes nothing.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

/// A job object holding the game and its descendants.
pub struct Job {
    handle: HANDLE,
}

// SAFETY: a job object handle is a plain kernel handle usable from any thread.
unsafe impl Send for Job {}

impl Job {
    /// Puts `child` in a new job object. A job that could not be created is tolerated: the
    /// result then does nothing.
    pub fn attach(child: &Child) -> Job {
        // SAFETY: both arguments may be null (no security attributes, no name).
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if !handle.is_null() {
            // SAFETY: `handle` is a live job object and the child's handle is valid while
            // `child` is borrowed.
            unsafe { AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) };
        }
        Job { handle }
    }

    /// Ends every process in the job with exit code 1.
    pub fn terminate(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: the handle is a live job object owned by `self`.
            unsafe { TerminateJobObject(self.handle, 1) };
        }
    }

    pub fn close(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: the handle is owned by `self` and nulled right after, so it closes once.
            unsafe { CloseHandle(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.close();
    }
}

/// Closes this process's stdout and stderr (the same pipe for the game), so a reader of that
/// pipe sees end of file while the process keeps running. Later writes do nothing.
pub fn close_std_streams() {
    let mut closed: Vec<HANDLE> = Vec::new();
    for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle takes a plain id and returns a handle value or null/invalid.
        let handle = unsafe { GetStdHandle(which) };
        if !handle.is_null() && handle != INVALID_HANDLE_VALUE && !closed.contains(&handle) {
            // SAFETY: the handle belongs to this process and each distinct value is closed once;
            // the std slots are cleared below so nothing uses it afterwards.
            unsafe { CloseHandle(handle) };
            closed.push(handle);
        }
        // SAFETY: setting a std slot to null is always allowed.
        unsafe { SetStdHandle(which, std::ptr::null_mut()) };
    }
}

/// Waits up to `limit` for process `pid` to end. `true` when it ended or does not exist (it
/// cannot be opened), `false` when it still runs after `limit`.
pub fn wait_for_process(pid: u32, limit: Duration) -> bool {
    // INFINITE is u32::MAX: a longer limit is cut one millisecond short of it.
    let millis = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX - 1);
    // SAFETY: takes plain values; a pid that cannot be opened returns null.
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return true;
    }
    // SAFETY: `handle` is a live process handle with SYNCHRONIZE access, opened above.
    let result = unsafe { WaitForSingleObject(handle, millis) };
    // SAFETY: `handle` is owned here and closed once.
    unsafe { CloseHandle(handle) };
    result == WAIT_OBJECT_0
}
