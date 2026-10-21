//! Windows calls.

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

/// The user's UI language id (LANGID), or `None` when Windows reports 0.
pub fn user_default_ui_language() -> Option<u32> {
    // SAFETY: the function takes no arguments and no pointers, and always succeeds.
    let id = unsafe { GetUserDefaultUILanguage() };
    (id != 0).then_some(u32::from(id))
}
