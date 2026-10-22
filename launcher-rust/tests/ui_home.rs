#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The Home page (chips, quick line, PLAY, update pill) and the language menu.

use std::path::Path;
use std::time::Duration;

use iced::time::Instant;
use iced_test::simulator::simulator;
use serde_json::json;
use spartanport_launcher::core::gpu::GpuRow;
use spartanport_launcher::core::i18n::tr;
use spartanport_launcher::core::update::Release;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::drawer::DrawerState;
use spartanport_launcher::ui::home;
use spartanport_launcher::ui::lang_menu::LangMessage;
use spartanport_launcher::ui::pages::Page;
use spartanport_launcher::ui::update::UpdateState;

/// A state over `dir` only, with the splash already gone.
fn state_in(dir: &Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    let mut state = State::new(spartanport_launcher::core::paths::Paths::from_env(
        dir, &env,
    ));
    state.splash.done = true;
    state
}

fn lang(state: &mut State, message: LangMessage) {
    let _ = app::update(state, Message::Lang(message));
}

fn tick(state: &mut State, at: Instant) {
    let _ = app::update(state, Message::Tick(at));
}

/// Clicks `label` in the current view and returns what the click emitted.
fn click(state: &State, label: &str) -> Vec<Message> {
    let mut ui = simulator(app::view(state));
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    ui.into_messages().collect()
}

#[test]
fn home_shows_play_and_the_four_chips() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    let chips = home::chips(&state);
    assert_eq!(chips[0], "• Graphics card: not checked");
    assert_eq!(chips[1], "✗ No eboot.bin in the game folder");
    assert_eq!(chips[2], "• No saves yet");
    assert_eq!(chips[3], "• Choose the game folder first.");
    assert_eq!(
        home::quick_line(&state),
        "FSR 3.1 (every GPU) · Quality (×1.5) · 1920 × 1080 · Unlimited · Windowed · Monitor Primary"
    );

    let mut ui = simulator(app::view(&state));
    assert!(ui.find("PLAY").is_ok());
    for chip in &chips {
        assert!(ui.find(chip.as_str()).is_ok(), "chip {chip:?} missing");
    }
    assert!(ui.find(home::quick_line(&state)).is_ok());
}

#[test]
fn the_home_art_falls_back_to_the_base_game() {
    let dir = tempfile::tempdir().unwrap();
    let (game, update) = (dir.path().join("game"), dir.path().join("update"));
    for folder in [&game, &update] {
        std::fs::create_dir_all(folder.join("sce_sys")).unwrap();
        std::fs::write(folder.join("eboot.bin"), b"elf").unwrap();
    }
    std::fs::write(game.join("sce_sys/pic1.png"), b"png").unwrap();
    let mut state = state_in(dir.path());
    state
        .settings
        .set("game_dir", game.display().to_string().into());
    state
        .settings
        .set("update_dir", update.display().to_string().into());
    home::load_home_art(&mut state);
    assert!(state.home_art.is_some(), "the update has no pic1.png");
}

#[test]
fn the_rust_only_home_and_splash_texts_have_portuguese() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    state.lang = "pt".into();
    // The wording of the Python's "• No saves yet: the game creates them in {}".
    assert_eq!(home::chips(&state)[2], "• Nenhum save ainda");
    state.splash.done = false;
    let mut ui = simulator(app::view(&state));
    assert!(ui.find("qualquer tecla pula").is_ok());
}

#[test]
fn the_graphics_card_chip_uses_the_python_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    state.gpus = vec![GpuRow {
        index: "0".into(),
        name: "NVIDIA GeForce RTX 4060".into(),
        kind: "discrete".into(),
        msaa: None,
        chosen: true,
    }];
    // `✓ Graphics card: {}` of the 2.6.0 tables, not a Vulkan version nobody checked.
    assert_eq!(
        home::chips(&state)[0],
        "✓ Graphics card: NVIDIA GeForce RTX 4060"
    );
    state.lang = "pt".into();
    assert_eq!(
        home::chips(&state)[0],
        "✓ Placa de vídeo: NVIDIA GeForce RTX 4060"
    );
}

#[test]
fn saves_chip_follows_the_savedata_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let saves = state
        .paths
        .user_dir(&state.settings.get_str("user_dir"))
        .join("savedata");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("slot0"), b"x").unwrap();
    home::refresh_facts(&mut state);
    assert!(home::chips(&state)[2].starts_with("✓ Saves found in "));
}

#[test]
fn hover_opens_after_450ms_and_leave_closes_after_300ms() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let t0 = Instant::now();
    state.now = t0;
    let ms = |n: u64| t0 + Duration::from_millis(n);

    lang(&mut state, LangMessage::Enter);
    tick(&mut state, ms(449));
    assert!(!state.lang_menu.open);
    tick(&mut state, ms(450));
    assert!(state.lang_menu.open);

    lang(&mut state, LangMessage::Leave);
    let left = state.now;
    tick(&mut state, left + Duration::from_millis(299));
    assert!(state.lang_menu.open);
    tick(&mut state, left + Duration::from_millis(300));
    assert!(!state.lang_menu.open);
}

#[test]
fn coming_back_before_300ms_keeps_the_menu_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    lang(&mut state, LangMessage::Toggle);
    assert!(state.lang_menu.open);
    let t0 = state.now;
    lang(&mut state, LangMessage::Leave);
    tick(&mut state, t0 + Duration::from_millis(200));
    lang(&mut state, LangMessage::Enter);
    tick(&mut state, t0 + Duration::from_millis(400));
    assert!(state.lang_menu.open);
}

#[test]
fn click_toggles_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    assert!(!state.lang_menu.open);
    for message in click(&state, "English ▾") {
        let _ = app::update(&mut state, message);
    }
    assert!(state.lang_menu.open);
    for message in click(&state, "English ▾") {
        let _ = app::update(&mut state, message);
    }
    assert!(!state.lang_menu.open);
}

#[test]
fn the_open_menu_lists_every_language_and_marks_the_current_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    lang(&mut state, LangMessage::Toggle);
    let mut ui = simulator(app::view(&state));
    assert!(ui.find("✓ English").is_ok());
    for name in [
        "Español",
        "Português (Brasil)",
        "Français",
        "Русский",
        "العربية",
        "日本語",
        "Bahasa Indonesia",
    ] {
        assert!(ui.find(name).is_ok(), "{name} missing");
    }
    let messages = {
        ui.click("Español").unwrap();
        ui.into_messages().collect::<Vec<_>>()
    };
    assert!(matches!(
        messages.as_slice(),
        [Message::Lang(LangMessage::Pick(code))] if code == "es"
    ));
}

#[test]
fn picking_portuguese_changes_the_tab_labels() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    lang(&mut state, LangMessage::Toggle);
    lang(&mut state, LangMessage::Pick("pt".to_string()));
    assert_eq!(state.lang, "pt");
    assert_eq!(state.settings.get_str("ui_language"), "pt");
    assert!(!state.lang_menu.open);
    assert_eq!(tr("pt", "Graphics"), "Gráficos");
    {
        let mut ui = simulator(app::view(&state));
        assert!(ui.find("Gráficos").is_ok());
        assert!(ui.find("Português (Brasil) ▾").is_ok());
    }

    lang(&mut state, LangMessage::Toggle);
    let mut ui = simulator(app::view(&state));
    for header in ["Américas", "Europa", "Ásia e Oriente Médio"] {
        assert!(ui.find(header).is_ok(), "{header} missing");
    }
}

#[test]
fn arabic_sets_rtl() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    assert!(!state.rtl);
    lang(&mut state, LangMessage::Pick("ar".to_string()));
    assert!(state.rtl);
    lang(&mut state, LangMessage::Pick("en".to_string()));
    assert!(!state.rtl);
    assert_eq!(state.settings.get("ui_language"), Some(&json!("en")));
}

#[test]
fn update_pill_only_when_available() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let pill = "Version 2.7.0 is available · Update";
    assert!(simulator(app::view(&state)).find(pill).is_err());
    state.update = UpdateState::Available(Release {
        version: "2.7.0".to_string(),
        url: None,
        page: String::new(),
    });
    assert!(simulator(app::view(&state)).find(pill).is_ok());
}

#[test]
fn play_gets_the_click_under_the_narrow_drawer() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let _ = app::update(&mut state, Message::ShowPage(Page::Graphics));
    // Let the slide finish.
    tick(&mut state, Instant::now() + Duration::from_secs(1));
    assert_eq!(state.drawer, DrawerState::Open(Page::Graphics));
    let messages = click(&state, "PLAY");
    assert!(messages.iter().any(|m| matches!(m, Message::Play)));
    // The dim layer under the drawer does not take the click too.
    assert!(!messages.iter().any(|m| matches!(m, Message::CloseDrawer)));
    for message in messages {
        let _ = app::update(&mut state, message);
    }
    // Without a game folder PLAY refuses and opens the Game page in the drawer.
    assert_eq!(state.drawer, DrawerState::Open(Page::Game));
}

#[test]
fn play_is_disabled_behind_the_wide_drawer() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let _ = app::update(&mut state, Message::ShowPage(Page::Trophies));
    tick(&mut state, Instant::now() + Duration::from_secs(1));
    let mut ui = simulator(app::view(&state));
    let _ = ui.click("PLAY");
    assert!(!ui
        .into_messages()
        .any(|message| matches!(message, Message::Play)));
}
