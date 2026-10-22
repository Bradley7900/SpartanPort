#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The splash phase: skip, fly to the corner, done.

use std::time::Duration;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Location, Modifiers};
use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{self, Message, State};

fn state_in(dir: &std::path::Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    State::new(Paths::from_env(dir, &env))
}

#[test]
fn a_key_skips_then_the_monogram_flies_and_the_splash_ends() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let start = state.splash.started;
    drop(app::view(&state));

    let _ = app::update(
        &mut state,
        Message::Key(
            Key::Named(Named::Space),
            Modifiers::default(),
            Location::Standard,
        ),
    );
    assert!(state.splash.skipped);

    let _ = app::update(
        &mut state,
        Message::Tick(start + Duration::from_millis(300)),
    );
    assert!(!state
        .splash
        .fly
        .is_animating(start + Duration::from_millis(300)));

    let flying = start + Duration::from_millis(900);
    let _ = app::update(&mut state, Message::Tick(flying));
    assert!(state.splash.fly.is_animating(flying));
    assert!(!state.splash.done);
    drop(app::view(&state));

    let _ = app::update(
        &mut state,
        Message::Tick(start + Duration::from_millis(1700)),
    );
    assert!(state.splash.done);
    drop(app::view(&state));
}
