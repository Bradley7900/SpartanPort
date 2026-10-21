//! The launcher's own update offer and install.

/// Update check and download progress; Task 34 adds the other states.
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    None,
}

/// Update events; Task 34 adds them.
#[derive(Debug, Clone)]
pub enum UpdateMessage {}
