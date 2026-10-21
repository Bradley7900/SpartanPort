//! The splash screen.

/// Splash phase; Task 22 adds the checks, the clock and the monogram flight.
#[derive(Debug, Clone, Default)]
pub struct SplashState {
    pub done: bool,
}

/// Splash events; Task 22 adds them.
#[derive(Debug, Clone)]
pub enum SplashMessage {}
