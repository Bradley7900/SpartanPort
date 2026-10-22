#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The splash phase: skip, fly to the corner, done.

use std::time::Duration;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Location, Modifiers};
use iced_test::simulator::simulator;
use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::splash::{Check, SplashMessage};

fn state_in(dir: &std::path::Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    State::new(Paths::from_env(dir, &env))
}

#[test]
fn the_splash_shows_the_three_checks_in_turn() {
    // Ruling R86: the checks answer at once, and each text still shows for a third of the 0.8 s.
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let start = state.splash.started;
    for check in [
        Check::Gpus(Vec::new()),
        Check::GameFolder(false),
        Check::Update(None),
    ] {
        let _ = app::update(&mut state, Message::Splash(SplashMessage::CheckDone(check)));
    }
    for (at, shown) in [
        (100, "Checking Vulkan graphics cards…"),
        (400, "Checking the game folder…"),
        (700, "Checking for updates…"),
    ] {
        let _ = app::update(&mut state, Message::Tick(start + Duration::from_millis(at)));
        let mut ui = simulator(app::view(&state));
        assert!(ui.find(shown).is_ok(), "{shown} missing at {at} ms");
        assert!(!state.splash.flying(), "flying at {at} ms");
    }
    let _ = app::update(
        &mut state,
        Message::Tick(start + Duration::from_millis(800)),
    );
    assert!(state.splash.flying());
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
