//! The launcher's own update offer and install.

use crate::core::update::Release;

/// Update check and download progress; Task 34 adds the other states.
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    None,
    /// A newer release exists; Task 34 adds the other states.
    Available(Release),
}

/// Update events; Task 34 adds the others.
#[derive(Debug, Clone)]
pub enum UpdateMessage {
    /// The Home pill was pressed.
    Install,
    /// The Check for updates button of the Advanced page (`manual`), Task 34 checks.
    Check { manual: bool },
}
