#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The first-run setup: when it opens, its steps, its edits and how it ends.

mod common;

use std::path::Path;

use common::{browsed, dump_with_patches, messages};
use iced_test::simulator::simulator;
use spartanport_launcher::core::i18n::tr;
use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::setup::{can_advance, folder_problems, SetupMessage, Step};

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

/// `dump_with_patches` plus the `sce_module` files the port needs, so every required file passes.
fn complete_dump(port: &Path) -> std::path::PathBuf {
    let dump = dump_with_patches(port);
    std::fs::create_dir_all(dump.join("sce_module")).unwrap();
    for name in ["libc.prx", "libSceFios2.prx"] {
        std::fs::write(dump.join("sce_module").join(name), b"prx").unwrap();
    }
    dump
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
    browsed(&mut state, "game_dir", &complete_dump(dir.path()));
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

#[test]
fn the_footer_is_mirrored_right_to_left() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    let skip_left_of_next = |state: &State| {
        let mut ui = simulator(app::view(state));
        let skip = ui.find(tr(&state.lang, "Skip")).unwrap().bounds();
        let next = ui.find(tr(&state.lang, "Next")).unwrap().bounds();
        skip.x < next.x
    };
    assert!(skip_left_of_next(&state));
    state.lang = "ar".into();
    state.rtl = true;
    assert!(!skip_left_of_next(&state), "Skip is at the right");
}

#[test]
fn next_waits_for_a_complete_game_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    assert!(!can_advance(&state));
    assert!(messages(&state, "Next").is_empty(), "Next has no on_press");
    let lines = folder_problems(&state, "game_dir");
    assert_eq!(
        lines.first().map(String::as_str),
        Some("Missing or empty files:")
    );
    assert!(
        lines.iter().any(|line| line.contains("eboot.bin")),
        "{lines:?}"
    );
    assert!(folder_problems(&state, "update_dir").is_empty());
    {
        let mut ui = simulator(app::view(&state));
        assert!(ui.find("Missing or empty files:").is_ok());
    }

    let dump = complete_dump(dir.path());
    browsed(&mut state, "game_dir", &dump);
    assert!(folder_problems(&state, "game_dir").is_empty());
    assert!(can_advance(&state));
    assert!(
        state.home.info.is_some(),
        "the Home facts follow the folder"
    );
    assert!(matches!(
        messages(&state, "Next").as_slice(),
        [Message::Setup(SetupMessage::Next)]
    ));
    setup(&mut state, SetupMessage::Next);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Gpu);
}

#[test]
fn next_is_ignored_while_the_folder_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    setup(&mut state, SetupMessage::Next);
    assert_eq!(state.setup.as_ref().unwrap().step, Step::Folders);
}

#[test]
fn the_update_folder_must_differ_from_the_game_folder() {
    // Review Focus: the update folder chosen first, the game folder pointed at it later.
    let dir = tempfile::tempdir().unwrap();
    let mut state = opened(dir.path());
    let dump = complete_dump(dir.path());
    browsed(&mut state, "update_dir", &dump);
    assert!(
        folder_problems(&state, "update_dir").is_empty(),
        "no game folder yet"
    );
    browsed(&mut state, "game_dir", &dump);
    assert_eq!(
        folder_problems(&state, "update_dir"),
        ["Choose two separate folders for the game and its update, or leave the update folder empty."]
    );
    assert!(!can_advance(&state));

    let other = dir.path().join("update");
    std::fs::create_dir_all(&other).unwrap();
    browsed(&mut state, "update_dir", &other);
    let lines = folder_problems(&state, "update_dir");
    assert_eq!(
        lines.first().map(String::as_str),
        Some("Missing or empty files:")
    );
    assert!(!can_advance(&state));
}
