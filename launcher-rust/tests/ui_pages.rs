#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The settings pages of the drawer (Tasks 24-31).

use spartanport_launcher::core::gpu::{display_label, parse_display_list, Display};
use spartanport_launcher::core::options::DISPLAY_MODES;
use spartanport_launcher::ui::app::{self, Message};
use spartanport_launcher::ui::pages::Page;
use spartanport_launcher::ui::widgets::select_message;

/// Helpers the page tests share.
mod common {
    use std::path::Path;

    use iced_test::simulator::simulator;
    use spartanport_launcher::core::paths::Paths;
    use spartanport_launcher::ui::app::{self, Message, State};
    use spartanport_launcher::ui::pages::Page;

    /// A state over `dir` only, with the splash already gone.
    pub fn state(dir: &Path) -> State {
        let root = dir.display().to_string();
        let env = move |key: &str| match key {
            "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
            _ => None,
        };
        let mut state = State::new(Paths::from_env(dir, &env));
        state.splash.done = true;
        state
    }

    pub fn open(state: &mut State, page: Page) {
        let _ = app::update(state, Message::ShowPage(page));
    }

    /// Clicks `label` in the current view and returns what the click emitted.
    pub fn messages(state: &State, label: &str) -> Vec<Message> {
        let mut ui = simulator(app::view(state));
        ui.click(label)
            .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
        ui.into_messages().collect()
    }

    /// Clicks `label` and feeds the emitted messages to `update`.
    pub fn click(state: &mut State, label: &str) {
        for message in messages(state, label) {
            let _ = app::update(state, message);
        }
    }
}

use common::{click, messages, open, state};

const TWO_DISPLAYS: &str =
    "0\tMonitor A\t1\t0\t0\t1920\t1080\n1\tMonitor B\t0\t1920\t0\t2560\t1440\n";

fn two_displays() -> Vec<Display> {
    parse_display_list(TWO_DISPLAYS)
}

#[test]
fn graphics_rows_change_the_right_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    open(&mut state, Page::Graphics);
    assert_eq!(state.ini.values["vsync"], "1");
    click(&mut state, "VSync");
    assert_eq!(state.ini.values["vsync"], "0");
    assert!(!state.settings.get_bool("hdr"));
    click(&mut state, "Allow HDR output");
    assert!(state.settings.get_bool("hdr"));
    // iced_test cannot open a pick_list: the pure mapping stands in for the click.
    let pick = select_message("display_mode", DISPLAY_MODES, "en", "Borderless").unwrap();
    assert!(
        matches!(&pick, Message::Set(key, value) if key == "display_mode" && value == "borderless")
    );
    let _ = app::update(&mut state, pick);
    assert_eq!(state.ini.values["display_mode"], "borderless");
    assert!(state.settings.get("display_mode").is_none());
}

#[test]
fn monitor_row_lists_the_probe_indices() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    let displays = two_displays();
    let _ = app::update(&mut state, Message::MonitorsListed(displays.clone()));
    assert_eq!(
        state.monitor_pairs,
        vec![
            (String::new(), "Primary (default)".to_string()),
            ("0".to_string(), display_label(&displays[0])),
            ("1".to_string(), display_label(&displays[1])),
        ]
    );
    let second = state.monitor_pairs[2].1.clone();
    let pick = state
        .monitor_pairs
        .iter()
        .find(|(_, label)| *label == second)
        .map(|(value, _)| Message::Set("monitor".into(), value.clone().into()))
        .unwrap();
    let _ = app::update(&mut state, pick);
    assert_eq!(state.settings.get_str("monitor"), "1");
}

#[test]
fn graphics_page_offers_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    open(&mut state, Page::Graphics);
    // The page lists the probe's monitors, a gap in the indices stays a gap.
    let gap = parse_display_list("0\tA\t1\t0\t0\t1920\t1080\n2\tB\t0\t0\t0\t1280\t720\n");
    let _ = app::update(&mut state, Message::MonitorsListed(gap));
    let values: Vec<&str> = state
        .monitor_pairs
        .iter()
        .map(|(v, _)| v.as_str())
        .collect();
    assert_eq!(values, ["", "0", "2"]);
    assert_eq!(state.monitor_pairs[1].1, "A · 1920 × 1080 · (primary)");
    assert_eq!(state.monitor_pairs[2].1, "B · 1280 × 720");

    // A saved choice comes back; one whose monitor is gone falls back to the primary default.
    state.settings.set("monitor", "2".into());
    let _ = app::update(&mut state, Message::MonitorsListed(two_displays()));
    assert_eq!(state.settings.get_str("monitor"), "");
    state.settings.set("monitor", "1".into());
    let _ = app::update(&mut state, Message::MonitorsListed(two_displays()));
    assert_eq!(state.settings.get_str("monitor"), "1");
    let _ = app::update(&mut state, Message::MonitorsListed(Vec::new()));
    assert_eq!(state.monitor_pairs.len(), 1);
    assert_eq!(state.settings.get_str("monitor"), "");

    // The screenshots button opens <saves>/screenshots.
    let sent = messages(&state, "Open the screenshots folder");
    let expected = state.paths.user_dir("").join("screenshots");
    assert!(matches!(sent.as_slice(), [Message::OpenPath(path)] if *path == expected));
}
