#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The first-run setup: when it opens, its steps, its edits and how it ends.

mod common;

use std::path::Path;

use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::setup::{SetupMessage, Step};

/// A state over `dir` only, as the program opens it, with the splash already gone.
fn opened(dir: &Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    let mut state = State::open(Paths::from_env(dir, &env));
    state.splash.done = true;
    state
}

fn setup(state: &mut State, message: SetupMessage) {
    let _ = app::update(state, Message::Setup(message));
}

#[test]
fn the_setup_opens_only_without_a_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    let state = opened(dir.path());
    assert_eq!(state.setup.as_ref().map(|s| s.step), Some(Step::Folders));

    for text in ["{}", "{not json"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), text).unwrap();
        assert!(opened(dir.path()).setup.is_none(), "{text:?}");
    }
    // `new` is what every other test builds on: never the setup.
    let dir = tempfile::tempdir().unwrap();
    assert!(common::state(dir.path()).setup.is_none());
}

#[test]
fn next_and_back_walk_the_three_steps() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    setup(&mut state, SetupMessage::Back);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Folders);
    setup(&mut state, SetupMessage::Next);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Gpu);
    setup(&mut state, SetupMessage::Next);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Video);
    setup(&mut state, SetupMessage::Next);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Video);
    setup(&mut state, SetupMessage::Back);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Gpu);
}
