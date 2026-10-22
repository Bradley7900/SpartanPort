#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The first-run setup: when it opens, its steps, its edits and how it ends.

mod common;

use std::path::Path;

use iced_test::simulator::simulator;
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

#[test]
fn the_setup_replaces_the_home_and_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    let state = opened(dir.path());
    let mut ui = simulator(app::view(&state));
    assert!(ui.find("Initial setup").is_ok());
    assert!(ui.find("Step 1 of 3 · Folders").is_ok());
    assert!(ui.find("Skip").is_ok());
    assert!(ui.find("Next").is_ok());
    assert!(ui.find("Back").is_err());
    assert!(ui.find("PLAY").is_err());
    assert!(ui.find("Graphics").is_err(), "the tab bar is not drawn");
    assert!(ui.find("English ▾").is_ok(), "the language button stays");
}

#[test]
fn the_last_step_offers_finish_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    state.setup.as_mut().unwrap().step = Step::Video;
    let mut ui = simulator(app::view(&state));
    assert!(ui.find("Step 3 of 3 · Video").is_ok());
    assert!(ui.find("Finish").is_ok());
    assert!(ui.find("Back").is_ok());
    assert!(ui.find("Next").is_err());
    assert!(matches!(
        common::messages(&state, "Back").as_slice(),
        [Message::Setup(SetupMessage::Back)]
    ));
}

#[test]
fn the_setup_reads_in_portuguese() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    state.lang = "pt".into();
    let mut ui = simulator(app::view(&state));
    assert!(ui.find("Configuração inicial").is_ok());
    assert!(ui.find("Passo 1 de 3 · Pastas").is_ok());
    assert!(ui.find("Pular").is_ok());
    assert!(ui.find("Avançar").is_ok());
}
