//! Windows calls.

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The user's UI language id (LANGID), or `None` when Windows reports 0.
pub fn user_default_ui_language() -> Option<u32> {
    // SAFETY: the function takes no arguments and no pointers, and always succeeds.
    let id = unsafe { GetUserDefaultUILanguage() };
    (id != 0).then_some(u32::from(id))
}

/// Hides the console window of the child.
pub fn prepare(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
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
