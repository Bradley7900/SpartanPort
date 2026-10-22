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

/// The labels the simulator finds on the rendered page, in a window tall enough for all rows.
fn page_has(state: &app::State, labels: &[&str]) {
    let mut ui = iced_test::simulator::Simulator::with_size(
        iced::Settings::default(),
        iced::Size::new(1280.0, 4000.0),
        app::view(state),
    );
    for label in labels {
        ui.find(*label)
            .unwrap_or_else(|error| panic!("find {label:?}: {error:?}"));
    }
}

#[test]
fn performance_page_offers_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    state.gpus = gpu_rows(&ARC_LAPTOP);
    open(&mut state, Page::Performance);
    assert_eq!(state.settings.get_str("no_msaa"), "");
    // The page shows the rows with their hints. The text inside a closed pick_list is not a
    // widget the simulator can find, so the options are checked through `rows` below.
    page_has(
        &state,
        &[
            "Game multisampling  ·  restart",
            performance::NO_MSAA_HINT,
            "Graphics card  ·  restart",
        ],
    );
    // iced_test cannot open a pick_list: the page builds its drop-downs from `rows`, so those
    // are what the page offers.
    let rows = performance::rows(&state, "en");
    let row = |key: &str| rows.iter().find(|row| row.key == key).unwrap().clone();
    let boxes = row("no_msaa");
    assert_eq!(boxes.pairs, performance::no_msaa_label(&state, "en"));
    assert_eq!(boxes.current, "");
    let pick = |row: &performance::SelectRow, label: &str| {
        let (value, _) = row.pairs.iter().find(|(_, l)| l == label).unwrap();
        Message::Set(row.key.into(), value.clone().into())
    };
    let _ = app::update(&mut state, pick(&boxes, "Off (fixes black lines)"));
    assert_eq!(state.settings.get_str("no_msaa"), "1");
    // The graphics card list offers Automatic and both cards, in the order of the probe.
    let cards = row("gpu_id");
    let labels: Vec<&str> = cards.pairs.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Automatic",
            "Intel(R) Arc(TM) Graphics",
            "NVIDIA GeForce RTX 4060 Laptop GPU"
        ]
    );
    // Choosing a card relabels Automatic at once and keeps the multisampling choice.
    let _ = app::update(&mut state, pick(&cards, "Intel(R) Arc(TM) Graphics"));
    assert_eq!(state.settings.get_str("gpu_id"), "0");
    let boxes = performance::rows(&state, "en")
        .into_iter()
        .find(|row| row.key == "no_msaa")
        .unwrap();
    assert_eq!(
        boxes.pairs[0].1,
        "Automatic (Intel(R) Arc(TM) Graphics: off)"
    );
    assert_eq!(boxes.current, "1");
    assert_eq!(state.settings.get_str("no_msaa"), "1");
    // The Portuguese page shows the translated title, hint and label.
    state.lang = "pt".into();
    let hint = spartanport_launcher::core::i18n::tr("pt", performance::NO_MSAA_HINT);
    assert_ne!(hint, performance::NO_MSAA_HINT);
    page_has(&state, &["Multisampling do jogo  ·  reiniciar", &hint]);
    let boxes = performance::rows(&state, "pt")
        .into_iter()
        .find(|row| row.key == "no_msaa")
        .unwrap();
    assert_eq!(
        boxes.pairs[0].1,
        "Automático (Intel(R) Arc(TM) Graphics: desligado)"
    );
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
    assert!(home::summary(&state).contains("3840 × 1080 · 32:9 (mod)"));
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

// --- Game page (Task 26) ---

use spartanport_launcher::core::i18n::tr;
use spartanport_launcher::core::options::LANGUAGES;
use spartanport_launcher::ui::home;
use spartanport_launcher::ui::pages::game::{self, BackupMessage};
use spartanport_launcher::ui::widgets::{ModalKind, ModalMessage};

fn sfo(entries: &[(&str, &str)]) -> Vec<u8> {
    let keys_off = 20 + 16 * entries.len();
    let (mut keys, mut values, mut table) = (Vec::new(), Vec::new(), Vec::new());
    for (key, value) in entries {
        let mut raw = value.as_bytes().to_vec();
        raw.push(0);
        table.extend_from_slice(&(keys.len() as u16).to_le_bytes());
        table.extend_from_slice(&0x204u16.to_le_bytes());
        table.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        table.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        table.extend_from_slice(&(values.len() as u32).to_le_bytes());
        keys.extend_from_slice(key.as_bytes());
        keys.push(0);
        values.extend_from_slice(&raw);
    }
    let values_off = keys_off + keys.len();
    let mut out = b"\0PSF".to_vec();
    out.extend_from_slice(&0x101u32.to_le_bytes());
    out.extend_from_slice(&(keys_off as u32).to_le_bytes());
    out.extend_from_slice(&(values_off as u32).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(keys);
    out.extend(values);
    out
}

/// A dump with `eboot.bin` and a tiny `param.sfo`, plus a test patch file under the port folder.
fn dump_with_patches(port: &std::path::Path) -> std::path::PathBuf {
    let dump = port.join("dump");
    std::fs::create_dir_all(dump.join("sce_sys")).unwrap();
    std::fs::write(dump.join("eboot.bin"), b"elf").unwrap();
    std::fs::write(
        dump.join("sce_sys").join("param.sfo"),
        sfo(&[
            ("TITLE_ID", "CUSA01623"),
            ("TITLE", "God of War III"),
            ("APP_VER", "01.02"),
        ]),
    )
    .unwrap();
    std::fs::create_dir_all(port.join("patches")).unwrap();
    std::fs::write(
        port.join("patches").join("CUSA01623.xml"),
        "<Patch><TitleID><ID>CUSA01623</ID></TitleID>\
         <Metadata Name=\"Fix Alpha\" Author=\"Ann\" Note=\"Fixes alpha.\" AppVer=\"01.02\" isEnabled=\"true\"><PatchList/></Metadata>\
         <Metadata Name=\"Resolution Patch - 3440x1440\" AppVer=\"01.02\"><PatchList/></Metadata>\
         <Metadata Name=\"Fix Other\" AppVer=\"01.01\" isEnabled=\"true\"><PatchList/></Metadata></Patch>",
    )
    .unwrap();
    dump
}

fn browsed(state: &mut app::State, key: &str, path: &std::path::Path) {
    let _ = app::update(
        state,
        Message::Browsed(key.into(), Some(path.to_path_buf())),
    );
}

#[test]
fn choosing_the_game_folder_refreshes_title_and_patches() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    assert_eq!(state.patches.title_id, None);
    let dump = dump_with_patches(dir.path());
    browsed(&mut state, "game_dir", &dump);
    assert_eq!(
        state.settings.get_str("game_dir"),
        dump.display().to_string()
    );
    assert_eq!(state.patches.title_id.as_deref(), Some("CUSA01623"));
    let names: Vec<&str> = state
        .patches
        .selectable
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    assert_eq!(names, ["Fix Alpha"]);
    assert!(state.patches.chosen.contains("Fix Alpha"));
    assert_eq!(state.patches.version, "01.02");
    assert_eq!(state.patches.xml_name, "CUSA01623.xml");
    assert_eq!(home::chips(&state)[1], "✓ CUSA01623 · v01.02");
    // Cancelling the dialog changes nothing.
    let _ = app::update(&mut state, Message::Browsed("game_dir".into(), None));
    assert_eq!(
        state.settings.get_str("game_dir"),
        dump.display().to_string()
    );
}

/// Like `click`, in a window tall enough to show the whole page (the drawer scrolls).
fn click_tall(state: &mut app::State, label: &str) {
    let mut ui = iced_test::simulator::Simulator::with_size(
        iced::Settings::default(),
        iced::Size::new(1280.0, 4000.0),
        app::view(state),
    );
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    let sent: Vec<Message> = ui.into_messages().collect();
    for message in sent {
        let _ = app::update(state, message);
    }
}

#[test]
fn unchecking_a_patch_lands_in_game_patches() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    browsed(&mut state, "game_dir", &dump_with_patches(dir.path()));
    open(&mut state, Page::Game);
    click_tall(&mut state, "Fix Alpha  (Ann)");
    assert!(!state.patches.chosen.contains("Fix Alpha"));
    let saved = state.settings.get("game_patches").unwrap();
    assert_eq!(saved["CUSA01623"], serde_json::json!([]));
    click_tall(&mut state, "Fix Alpha  (Ann)");
    let saved = state.settings.get("game_patches").unwrap();
    assert_eq!(saved["CUSA01623"], serde_json::json!(["Fix Alpha"]));
}

#[test]
fn restore_asks_first_and_is_blocked_while_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    let user = state.paths.user_dir("");
    let backup = user.join("savedata_backups").join("20261009-180000");
    std::fs::create_dir_all(&backup).unwrap();
    std::fs::write(backup.join("data.bin"), b"old").unwrap();
    std::fs::create_dir_all(user.join("savedata")).unwrap();
    std::fs::write(user.join("savedata").join("data.bin"), b"new").unwrap();

    // Nothing chosen: nothing happens.
    let _ = app::update(&mut state, Message::Backup(BackupMessage::Restore));
    assert!(state.modal.is_none());

    let _ = app::update(&mut state, Message::Backup(BackupMessage::Refresh));
    assert_eq!(state.backups, ["20261009-180000"]);
    assert_eq!(state.backup_choice, "20261009-180000");
    // Task 33 adds the running states; until then only an idle game exists.
    assert!(!game::restore_blocked(&state));

    let _ = app::update(&mut state, Message::Backup(BackupMessage::Restore));
    let asked = state.modal.clone().unwrap();
    assert!(matches!(
        &asked.kind,
        ModalKind::YesNo {
            yes: Message::Backup(BackupMessage::RestoreConfirmed),
            no: Message::Modal(ModalMessage::Dismiss)
        }
    ));
    assert_eq!(
        asked.text,
        "Replace the current saves with the backup 20261009-180000? The current saves are backed up first."
    );
    // Nothing was restored yet.
    assert_eq!(
        std::fs::read(user.join("savedata/data.bin")).unwrap(),
        b"new"
    );

    // Yes closes the dialog and starts the restore in the background.
    let _ = app::update(&mut state, Message::Backup(BackupMessage::RestoreConfirmed));
    assert!(state.modal.is_none());
    // The background work runs on its own thread: wait for the saves to come back.
    let saves = user.join("savedata/data.bin");
    for _ in 0..200 {
        if std::fs::read(&saves).is_ok_and(|bytes| bytes == b"old") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(std::fs::read(&saves).unwrap(), b"old");
    let _ = app::update(
        &mut state,
        Message::Backup(BackupMessage::Restored(Ok(Some("20261009-181000".into())))),
    );
    let done = state.modal.clone().unwrap();
    assert!(matches!(done.kind, ModalKind::Info));
    assert_eq!(
        done.text,
        "Saves restored from 20261009-180000. Previous saves: 20261009-181000."
    );
    let _ = app::update(
        &mut state,
        Message::Backup(BackupMessage::Restored(Err("disk full".into()))),
    );
    let failed = state.modal.clone().unwrap();
    assert!(matches!(failed.kind, ModalKind::Error));
    assert_eq!(failed.text, "Could not restore: disk full");
    assert!(matches!(
        game::restore_saves(&user, "nope"),
        Err(text) if text.contains("nope")
    ));
}

#[test]
fn brazilian_portuguese_is_offered_with_its_system_id() {
    let offered = LANGUAGES
        .iter()
        .find(|choice| choice.value == "17")
        .unwrap();
    assert_eq!(offered.en, "Portuguese (Brazil)");
}

#[test]
fn choosing_brazilian_portuguese_starts_the_game_with_id_17() {
    // The Language row stores the system id the game is started with (Task 33 passes it on).
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    let pick = select_message("language", LANGUAGES, "en", "Portuguese (Brazil)").unwrap();
    let _ = app::update(&mut state, pick);
    assert_eq!(state.settings.get_str("language"), "17");
}

#[test]
fn brazilian_portuguese_label_is_translated_in_portuguese_and_spanish() {
    assert_eq!(tr("pt", "Portuguese (Brazil)"), "Português (Brasil)");
    assert_eq!(tr("es", "Portuguese (Brazil)"), "Portugués (Brasil)");
    assert_eq!(tr("id", "Portuguese (Brazil)"), "Portugis (Brasil)");
}

#[test]
fn the_note_names_the_game_menus() {
    let note = game::LANGUAGE_NOTE;
    assert!(note.contains("Options → Game Settings → Game Text"));
    assert!(note.contains("Options → Audio → Speech"));
    assert!(note.starts_with("Used on the game's first start."));
}

#[test]
fn the_note_uses_the_game_menu_names_of_each_language() {
    let names = [
        ("pt", "Texto de jogo", "Falas"),
        ("es", "Texto del juego", "Diálogos"),
        ("pl", "Tekst gry", "Dialogi"),
        ("fr", "Texte du jeu", "Voix"),
        ("de", "Spieltext", "Sprachausgabe"),
        ("it", "Testo di gioco", "Dialoghi"),
    ];
    for (language, text, speech) in names {
        let translated = tr(language, game::LANGUAGE_NOTE);
        assert!(translated.contains(text), "{language}: {translated}");
        assert!(translated.contains(speech), "{language}: {translated}");
    }
}

// --- Trophies page (Task 27) ---

use spartanport_launcher::core::i18n::trophy_text;
use spartanport_launcher::core::trophies::TROPHY_COUNT;
use spartanport_launcher::ui::pages::trophies::{self, Filter, TrophiesMessage};

/// A 4x4 solid PNG.
fn tiny_png(shade: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, shade, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A `trophy00.trp` with `TROP000.PNG`... as the core reader expects it.
fn trp_package(count: usize) -> Vec<u8> {
    let element = 0x30;
    let table_end = 0x60 + element * count;
    let mut file = vec![0u8; table_end];
    file[..4].copy_from_slice(&[0xdc, 0xa2, 0x4d, 0x00]);
    file[0x10..0x14].copy_from_slice(&(count as u32).to_be_bytes());
    file[0x14..0x18].copy_from_slice(&(element as u32).to_be_bytes());
    for index in 0..count {
        let png = tiny_png(index as u8 * 5);
        let at = 0x60 + index * element;
        let name = format!("TROP{index:03}.PNG");
        file[at..at + name.len()].copy_from_slice(name.as_bytes());
        let offset = file.len() as u64;
        file[at + 0x20..at + 0x28].copy_from_slice(&offset.to_be_bytes());
        file[at + 0x28..at + 0x30].copy_from_slice(&(png.len() as u64).to_be_bytes());
        file.extend(png);
    }
    file
}

/// A state whose game folder holds a 35-trophy package and whose saves unlock trophies 0 and 3.
fn state_with_trophies(dir: &std::path::Path) -> app::State {
    let mut state = state(dir);
    state.lang = "en".into();
    let game = dir.join("game");
    std::fs::create_dir_all(game.join("sce_sys/trophy")).unwrap();
    std::fs::write(
        game.join("sce_sys/trophy/trophy00.trp"),
        trp_package(TROPHY_COUNT),
    )
    .unwrap();
    let _ = app::update(
        &mut state,
        Message::Set("game_dir".into(), game.display().to_string().into()),
    );
    let user = state.paths.user_dir("");
    std::fs::create_dir_all(&user).unwrap();
    std::fs::write(user.join("trophies.dat"), [0b1001u8, 0, 0, 0]).unwrap();
    state
}

fn load_trophies(state: &mut app::State) {
    let view = trophies::load(&state.paths, &state.settings);
    let _ = app::update(
        state,
        Message::Trophies(TrophiesMessage::Loaded(Box::new(view))),
    );
}

fn tall_has(state: &app::State, label: &str) -> bool {
    let mut ui = iced_test::simulator::Simulator::with_size(
        iced::Settings::default(),
        iced::Size::new(1280.0, 6000.0),
        app::view(state),
    );
    ui.find(label).is_ok()
}

#[test]
fn trophies_page_lists_35_cards_from_a_test_package() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_with_trophies(dir.path());
    open(&mut state, Page::Trophies);
    load_trophies(&mut state);
    assert_eq!(state.trophies.icons.len(), TROPHY_COUNT);
    assert_eq!(state.trophies.locked_icons.len(), TROPHY_COUNT);
    assert_eq!(state.trophies.tiers.len(), TROPHY_COUNT);
    assert_eq!(
        state.trophies.unlocked.iter().copied().collect::<Vec<_>>(),
        [0, 3]
    );
    assert!(!state.trophies.loading);
    assert!(tall_has(&state, "2 of 35 unlocked"));
    assert!(tall_has(&state, &trophy_text("en", 0).0));
    assert!(tall_has(&state, &trophy_text("en", TROPHY_COUNT - 1).0));
    assert!(tall_has(
        &state,
        "Only trophies unlocked with this port are listed; the ones earned on a PlayStation are not known to it."
    ));
}

#[test]
fn trophies_page_without_a_package_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    open(&mut state, Page::Trophies);
    load_trophies(&mut state);
    assert!(state.trophies.icons.is_empty());
    assert!(tall_has(
        &state,
        "No trophy list: choose the game folder on the Game page (its sce_sys/trophy/trophy00.trp is read)."
    ));
}

#[test]
fn filter_unlocked_hides_locked() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_with_trophies(dir.path());
    open(&mut state, Page::Trophies);
    load_trophies(&mut state);
    let (first, second, other) = (
        trophy_text("en", 0).0,
        trophy_text("en", 3).0,
        trophy_text("en", 1).0,
    );
    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::Filter(Filter::Unlocked)),
    );
    assert!(tall_has(&state, &first) && tall_has(&state, &second));
    assert!(!tall_has(&state, &other));
    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::Filter(Filter::Locked)),
    );
    assert!(!tall_has(&state, &first) && !tall_has(&state, &second));
    assert!(tall_has(&state, &other));
    // A reload keeps the chosen filter.
    load_trophies(&mut state);
    assert_eq!(state.trophies.filter, Filter::Locked);
}

#[test]
fn reset_asks_and_relocks() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_with_trophies(dir.path());
    open(&mut state, Page::Trophies);
    // Nothing unlocked is known before the first load.
    let _ = app::update(&mut state, Message::Trophies(TrophiesMessage::Reset));
    let none = state.modal.clone().unwrap();
    assert!(matches!(none.kind, ModalKind::Info));
    assert_eq!(none.text, "No trophy is unlocked.");

    load_trophies(&mut state);
    let _ = app::update(&mut state, Message::Trophies(TrophiesMessage::Reset));
    let asked = state.modal.clone().unwrap();
    assert!(matches!(
        &asked.kind,
        ModalKind::YesNo {
            yes: Message::Trophies(TrophiesMessage::ResetConfirmed),
            no: Message::Modal(ModalMessage::Dismiss)
        }
    ));
    assert_eq!(
        asked.text,
        "Lock all 2 unlocked trophies again? A copy is kept as trophies.dat.bak."
    );
    let dat = state.paths.user_dir("").join("trophies.dat");
    assert!(dat.is_file());

    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::ResetConfirmed),
    );
    assert!(state.modal.is_none());
    // The reset runs on its own thread: wait for the file to move.
    for _ in 0..200 {
        if !dat.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!dat.exists());
    assert!(dat.with_file_name("trophies.dat.bak").is_file());
    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::ResetDone(Ok(2))),
    );
    load_trophies(&mut state);
    assert!(state.trophies.unlocked.is_empty());
    assert!(tall_has(&state, "0 of 35 unlocked"));

    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::ResetDone(Err("disk full".into()))),
    );
    let failed = state.modal.clone().unwrap();
    assert!(matches!(failed.kind, ModalKind::Error));
    assert_eq!(failed.text, "Could not reset the trophies: disk full");
}

#[test]
fn opening_the_page_and_the_buttons_start_a_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    open(&mut state, Page::Trophies);
    assert!(state.trophies.loading);
    load_trophies(&mut state);
    assert!(!state.trophies.loading);
    click(&mut state, "Refresh");
    assert!(state.trophies.loading);
    assert!(messages(&state, "Reset trophies")
        .iter()
        .any(|m| matches!(m, Message::Trophies(TrophiesMessage::Reset))));
}
