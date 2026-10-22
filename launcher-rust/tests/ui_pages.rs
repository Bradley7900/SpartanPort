#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The settings pages of the drawer (Tasks 24-31).

use spartanport_launcher::core::gpu::{display_label, parse_display_list, Display, GpuRow};
use spartanport_launcher::core::options::DISPLAY_MODES;
use spartanport_launcher::ui::app::{self, Message};
use spartanport_launcher::ui::pages::{performance, Page};
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

fn set(state: &mut app::State, key: &str, value: &str) {
    let _ = app::update(state, Message::Set(key.into(), value.into()));
}

fn values(pairs: &[(String, String)]) -> Vec<&str> {
    pairs.iter().map(|(value, _)| value.as_str()).collect()
}

#[test]
fn aspect_follows_and_filters_the_render_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    assert_eq!(state.ini.values["render_resolution"], "native");
    assert_eq!(state.aspect, "16:9");
    let _ = app::update(&mut state, Message::Aspect("16:10".into()));
    assert_eq!(state.ini.values["render_resolution"], "1920x1200");
    assert_eq!(
        values(&performance::aspect_resolution_options(&state)),
        ["1280x800", "1440x900", "1920x1200", "2560x1600"]
    );
    set(&mut state, "render_resolution", "3440x1440");
    assert_eq!(state.aspect, "21:9");
    assert!(performance::aspect_options(&state).contains(&"4:3".to_string()));
    // Confirmed in game: not beta, so no suffix.
    let options = performance::aspect_resolution_options(&state);
    let label = options.iter().find(|(v, _)| v == "3440x1440").unwrap();
    assert_eq!(label.1, "3440 × 1440");
}

type GpuTuple = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const ARC_LAPTOP: [GpuTuple; 2] = [
    ("0", "Intel(R) Arc(TM) Graphics", "integrated", "off", "0"),
    (
        "1",
        "NVIDIA GeForce RTX 4060 Laptop GPU",
        "discrete",
        "on",
        "1",
    ),
];

/// Rows as `spartanport-gpu-capabilities --list` prints them: index, name, type, default, pick.
fn gpu_rows(rows: &[GpuTuple]) -> Vec<GpuRow> {
    rows.iter()
        .map(|(index, name, kind, msaa, chosen)| GpuRow {
            index: index.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            msaa: (!msaa.is_empty()).then(|| msaa.to_string()),
            chosen: *chosen == "1",
        })
        .collect()
}

#[test]
fn no_msaa_auto_label_names_the_card_and_the_decision() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.gpus = gpu_rows(&ARC_LAPTOP);
    let options = performance::no_msaa_label(&state, "en");
    assert_eq!(values(&options), ["", "1", "0"]);
    assert_eq!(
        options[0].1,
        "Automatic (NVIDIA GeForce RTX 4060 Laptop GPU: on)"
    );
    assert_eq!(options[1].1, "Off (fixes black lines)");
    assert_eq!(options[2].1, "On");
    state.settings.set("gpu_id", "0".into());
    assert_eq!(
        performance::no_msaa_label(&state, "en")[0].1,
        "Automatic (Intel(R) Arc(TM) Graphics: off)"
    );
    // An old tool, none at all, or a spelling this launcher does not know: the plain label.
    state.gpus = gpu_rows(&[("0", "AMD Radeon RX 6600", "discrete", "", "0")]);
    assert_eq!(performance::no_msaa_label(&state, "en")[0].1, "Automatic");
    state.gpus = Vec::new();
    state.settings.set("gpu_id", "".into());
    assert_eq!(performance::no_msaa_label(&state, "en")[0].1, "Automatic");
    state.gpus = gpu_rows(&[("0", "x", "discrete", "OFF", "1")]);
    assert_eq!(performance::no_msaa_label(&state, "en")[0].1, "Automatic");
    state.gpus = gpu_rows(&ARC_LAPTOP);
    state.settings.set("gpu_id", "0".into());
    assert_eq!(
        performance::no_msaa_label(&state, "pt")[0].1,
        "Automático (Intel(R) Arc(TM) Graphics: desligado)"
    );
    state.settings.set("gpu_id", "1".into());
    assert_eq!(
        performance::no_msaa_label(&state, "pt")[0].1,
        "Automático (NVIDIA GeForce RTX 4060 Laptop GPU: ligado)"
    );
}

#[test]
fn performance_page_offers_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    state.gpus = gpu_rows(&ARC_LAPTOP);
    open(&mut state, Page::Performance);
    assert_eq!(state.settings.get_str("no_msaa"), "");
    // iced_test cannot open a pick_list: the label -> value pairs stand in for the click.
    let pick = |pairs: &[(String, String)], key: &str, label: &str| {
        let (value, _) = pairs.iter().find(|(_, l)| l == label).unwrap();
        Message::Set(key.into(), value.clone().into())
    };
    let boxes = performance::no_msaa_label(&state, "en");
    let _ = app::update(
        &mut state,
        pick(&boxes, "no_msaa", "Off (fixes black lines)"),
    );
    assert_eq!(state.settings.get_str("no_msaa"), "1");
    // The graphics card list offers Automatic and both cards; Automatic of the box follows it.
    let cards = performance::gpu_pairs(&state);
    assert_eq!(
        cards,
        vec![
            (String::new(), "Automatic".to_string()),
            ("0".to_string(), "Intel(R) Arc(TM) Graphics".to_string()),
            (
                "1".to_string(),
                "NVIDIA GeForce RTX 4060 Laptop GPU".to_string()
            ),
        ]
    );
    let _ = app::update(
        &mut state,
        pick(&cards, "gpu_id", "Intel(R) Arc(TM) Graphics"),
    );
    assert_eq!(state.settings.get_str("gpu_id"), "0");
    assert_eq!(
        performance::no_msaa_label(&state, "en")[0].1,
        "Automatic (Intel(R) Arc(TM) Graphics: off)"
    );
    assert_eq!(state.settings.get_str("no_msaa"), "1");
    // The rows are on the page: the Shaders checks toggle their ini and app keys.
    let async_shaders = state.ini.values["async_shaders"].clone();
    click(&mut state, "Compile new shaders in the background");
    assert_ne!(state.ini.values["async_shaders"], async_shaders);
    let warmup = state.settings.get_bool("background_warmup");
    click(&mut state, "Load shader cache in the background");
    assert_eq!(state.settings.get_bool("background_warmup"), !warmup);
}

fn write_mod_patch(folder: &std::path::Path, size: &str) {
    std::fs::write(
        folder.join("u.xml"),
        format!(
            "<Patch><TitleID><ID>CUSA01623</ID></TitleID>\
             <Metadata Name=\"Resolution Patch - {size}\" AppVer=\"01.02\"><PatchList/></Metadata></Patch>"
        ),
    )
    .unwrap();
}

#[test]
fn a_saved_mod_resolution_stays_chosen_while_its_mod_is_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    let builtin = performance::resolution_options(&state).len();
    let patches = state.paths.mods_dir("").join("Ultrawide").join("patches");
    std::fs::create_dir_all(&patches).unwrap();
    // 3440x1440 is built in: the mod adds nothing new and the saved size stays chosen.
    write_mod_patch(&patches, "3440x1440");
    set(&mut state, "render_resolution", "3440x1440");
    performance::refresh_mod_resolutions(&mut state);
    assert_eq!(performance::resolution_options(&state).len(), builtin);
    assert_eq!(state.ini.values["render_resolution"], "3440x1440");

    // A size that is not built in carries the (mod) label while its mod is on.
    write_mod_patch(&patches, "3840x1080");
    set(&mut state, "render_resolution", "3840x1080");
    performance::refresh_mod_resolutions(&mut state);
    let options = performance::resolution_options(&state);
    assert_eq!(options.len(), builtin + 1);
    let label = options.iter().find(|(v, _)| v == "3840x1080").unwrap();
    assert_eq!(label.1, "3840 × 1080 · 32:9 (mod)");
    assert_eq!(state.ini.values["render_resolution"], "3840x1080");
    assert_eq!(state.aspect, "32:9");
    let listed = performance::aspect_resolution_options(&state);
    let label = listed.iter().find(|(v, _)| v == "3840x1080").unwrap();
    assert_eq!(label.1, "3840 × 1080 (mod)");
    // Unticked on the Mods page: the size is gone from the list.
    std::fs::write(
        state.paths.mods_json(),
        r#"{"disabled": ["Ultrawide"], "order": []}"#,
    )
    .unwrap();
    performance::refresh_mod_resolutions(&mut state);
    assert_eq!(performance::resolution_options(&state).len(), builtin);
    // Ticked again, then "Load mods" off.
    std::fs::remove_file(state.paths.mods_json()).unwrap();
    performance::refresh_mod_resolutions(&mut state);
    assert_eq!(performance::resolution_options(&state).len(), builtin + 1);
    state.settings.set("mods_enabled", false.into());
    performance::refresh_mod_resolutions(&mut state);
    assert_eq!(performance::resolution_options(&state).len(), builtin);
}
