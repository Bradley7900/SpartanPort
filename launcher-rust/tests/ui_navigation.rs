#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tab bar, drawer and keyboard navigation.
//!
//! Clicks go through `iced_test::simulator` over `app::view`. Key presses in the simulator reach
//! the widget tree, not the application's keyboard subscription, so the keyboard tests feed
//! `Message::Key(...)` to `app::update` directly, as the subscription does at run time.

use std::path::Path;
use std::time::Duration;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Location, Modifiers};
use iced::time::Instant;
use iced::{Point, Size};
use iced_test::core::Settings;
use iced_test::simulator::{self, simulator, Simulator};
use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{
    self, home_interactive_enabled, home_interactive_opacity, key_message, Message, State,
};
use spartanport_launcher::ui::drawer::{self, drawer_width, DrawerState};
use spartanport_launcher::ui::pages::Page;
use spartanport_launcher::VERSION;

/// The page order of the Global Constraints.
const ORDER: [Page; 9] = [
    Page::Home,
    Page::Graphics,
    Page::Performance,
    Page::Game,
    Page::Trophies,
    Page::Controls,
    Page::Mods,
    Page::Advanced,
    Page::Log,
];

/// The pages whose drawer takes 72 % of the window.
const WIDE: [Page; 4] = [Page::Trophies, Page::Controls, Page::Mods, Page::Log];

/// A state over `dir` only: data, ini and the legacy config folder all live there.
fn state_in(dir: &Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    let mut state = State::new(Paths::from_env(dir, &env));
    // These tests are about the tabs, not the splash that covers them at start.
    state.splash.done = true;
    state
}

fn click(state: &mut State, label: &str) {
    let mut ui = simulator(app::view(state));
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    for message in ui.into_messages() {
        let _ = app::update(state, message);
    }
}

/// Presses the left button at `(x, y)` of a 1280 × 800 window and applies what it emitted.
fn click_at(state: &mut State, x: f32, y: f32) {
    let mut ui = Simulator::with_size(
        Settings::default(),
        Size::new(1280.0, 800.0),
        app::view(state),
    );
    ui.point_at(Point::new(x, y));
    let _ = ui.simulate(simulator::click());
    for message in ui.into_messages() {
        let _ = app::update(state, message);
    }
}

fn press(state: &mut State, key: Named) {
    let _ = app::update(
        state,
        Message::Key(Key::Named(key), Modifiers::default(), Location::Standard),
    );
}

#[test]
fn every_tab_opens_its_drawer_with_the_right_width() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    assert_eq!(state.lang, "en");
    assert_eq!(Page::ALL, ORDER);
    assert!(home_interactive_enabled(&state));
    assert_eq!(home_interactive_opacity(&state), 1.0);
    for page in Page::ALL.into_iter().filter(|page| *page != Page::Home) {
        click(&mut state, page.label_en());
        assert_eq!(state.drawer, DrawerState::Open(page));
        assert_eq!(state.page, page);
        let wide = WIDE.contains(&page);
        assert_eq!(page.wide(), wide, "{page:?}");
        let expected = 1280.0 * if wide { 0.72 } else { 0.46 };
        let width = drawer_width(&state);
        assert!((width - expected).abs() < 0.01, "{page:?}: {width}");
        // The Home block fades to 35 % and stops responding only behind a wide drawer.
        assert_eq!(home_interactive_enabled(&state), !wide, "{page:?}");
        let opacity = if wide { 0.35 } else { 1.0 };
        assert_eq!(home_interactive_opacity(&state), opacity, "{page:?}");
    }
}

#[test]
fn escape_closes() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    click(&mut state, "Graphics");
    assert_eq!(state.drawer, DrawerState::Open(Page::Graphics));
    press(&mut state, Named::Escape);
    assert_eq!(state.drawer, DrawerState::Closed);
    assert_eq!(state.page, Page::Home);
}

#[test]
fn arrows_cycle_pages_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    assert_eq!(state.page, Page::Home);

    let mut forward = Vec::new();
    for _ in 0..ORDER.len() {
        press(&mut state, Named::ArrowRight);
        forward.push(state.page);
        let expected = if state.page == Page::Home {
            DrawerState::Closed
        } else {
            DrawerState::Open(state.page)
        };
        assert_eq!(state.drawer, expected);
    }
    let mut wrapped = ORDER[1..].to_vec();
    wrapped.push(Page::Home);
    assert_eq!(forward, wrapped);

    let mut backward = Vec::new();
    for _ in 0..ORDER.len() {
        press(&mut state, Named::ArrowLeft);
        backward.push(state.page);
    }
    let reversed: Vec<Page> = ORDER.iter().rev().copied().collect();
    assert_eq!(backward, reversed);
}

#[test]
fn home_tab_closes_the_drawer() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    click(&mut state, "Mods");
    assert_eq!(state.drawer, DrawerState::Open(Page::Mods));
    click(&mut state, "Home");
    assert_eq!(state.drawer, DrawerState::Closed);
    assert_eq!(state.page, Page::Home);
}

#[test]
fn enter_on_home_sends_play() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    let enter = Key::Named(Named::Enter);

    assert!(matches!(key_message(&state, &enter), Some(Message::Play)));
    let task = app::update(
        &mut state,
        Message::Key(enter.clone(), Modifiers::default(), Location::Standard),
    );
    assert_eq!(task.units(), 1);

    click(&mut state, "Graphics");
    assert!(key_message(&state, &enter).is_none());
    let task = app::update(
        &mut state,
        Message::Key(enter, Modifiers::default(), Location::Standard),
    );
    assert_eq!(task.units(), 0);
    assert_eq!(state.drawer, DrawerState::Open(Page::Graphics));
}

#[test]
fn background_click_closes_but_the_drawer_body_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_in(dir.path());
    click(&mut state, "Graphics");
    // Let the slide finish so the drawer sits at x = 0 .. 588.8.
    let _ = app::update(
        &mut state,
        Message::Tick(Instant::now() + Duration::from_secs(1)),
    );

    click_at(&mut state, 200.0, 300.0);
    assert_eq!(state.drawer, DrawerState::Open(Page::Graphics));

    click_at(&mut state, 1200.0, 300.0);
    assert_eq!(state.drawer, DrawerState::Closed);
}

#[test]
fn rtl_puts_the_drawer_on_the_right() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"ui_language": "ar"}"#).unwrap();
    let mut state = state_in(dir.path());
    assert_eq!(state.lang, "ar");
    assert!(state.rtl);
    // The tab labels are in Arabic, so the page is opened by its message, not by its label.
    let _ = app::update(&mut state, Message::ShowPage(Page::Graphics));
    let _ = app::update(
        &mut state,
        Message::Tick(Instant::now() + Duration::from_secs(1)),
    );
    let mut ui = Simulator::with_size(
        Settings::default(),
        Size::new(1280.0, 800.0),
        app::view(&state),
    );
    let panel = ui.find(drawer::panel_id()).unwrap().bounds();
    assert!(panel.x >= 640.0, "drawer at x = {}", panel.x);
    assert!((panel.x + panel.width - 1280.0).abs() < 1.0, "{panel:?}");
    // The bar is mirrored too: the version sits at the left, the tabs at the right.
    let version = ui.find(format!("v{VERSION}")).unwrap().bounds();
    assert!(version.x < 640.0, "version at x = {}", version.x);
    // Arabic needs advanced shaping: iced's default (`Auto`) gives it to every non-ASCII text.
    assert!(matches!(
        iced::widget::text::Shaping::default(),
        iced::widget::text::Shaping::Auto | iced::widget::text::Shaping::Advanced
    ));
}
