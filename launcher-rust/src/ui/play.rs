//! PLAY, STOP and the running game.

/// The game process as the launcher sees it; Task 33 adds the running states.
#[derive(Debug, Default)]
pub enum GameState {
    #[default]
    Idle,
}

/// Game process events; Task 33 adds them.
#[derive(Debug, Clone)]
pub enum GameMessage {}
