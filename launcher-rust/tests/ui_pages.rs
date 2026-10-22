#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The settings pages of the drawer (Tasks 24-31).

mod common;

use common::{
    browsed, click, dump_with_patches, gpu_rows, load_mods, load_trophies, make_mod, messages,
    open, state, state_with_trophies, two_displays, write_mod_patch, ARC_LAPTOP,
};
use spartanport_launcher::core::gpu::{display_label, parse_display_list};
use spartanport_launcher::core::options::DISPLAY_MODES;
use spartanport_launcher::ui::app::{self, Message};
use spartanport_launcher::ui::pages::{performance, Page};
use spartanport_launcher::ui::widgets::select_message;

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
    // An idle game does not block it; `ui_play.rs` covers a running one.
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
use spartanport_launcher::ui::pages::trophies::{Filter, TrophiesMessage};

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

// --- Controls page (Task 28) ---

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Location, Modifiers};
use iced::mouse;
use spartanport_launcher::core::controls::{Kind, PAD_STYLES};
use spartanport_launcher::ui::pages::controls::{
    self, ControlsMessage, ControlsView, Device, MouseInput, PadChoice, CAPTURE_SECONDS,
};

fn controls_update(state: &mut app::State, message: ControlsMessage) {
    let _ = app::update(state, Message::Controls(message));
}

/// Clicks the key box of `button`: the page now waits for an input for it.
fn capture(state: &mut app::State, button: &str) {
    controls_update(state, ControlsMessage::Capture(button.into()));
}

/// A key press as the keyboard subscription delivers it.
fn press_key(state: &mut app::State, key: Key, location: Location) {
    let _ = app::update(state, Message::Key(key, Modifiers::default(), location));
}

fn char_key(text: &str) -> Key {
    Key::Character(text.into())
}

/// Like `messages`, in a window tall enough to show the whole page (the drawer scrolls).
fn tall_messages(state: &app::State, label: &str) -> Vec<Message> {
    let mut ui = iced_test::simulator::Simulator::with_size(
        iced::Settings::default(),
        iced::Size::new(1280.0, 4000.0),
        app::view(state),
    );
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    ui.into_messages().collect()
}

fn controls_state(dir: &std::path::Path) -> app::State {
    let mut state = state(dir);
    state.lang = "en".into();
    open(&mut state, Page::Controls);
    state
}

#[test]
fn pressed_keys_become_sdl_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    // The Tk keysyms of the Python test (q, Q, 7, F12, space, Shift_L, Control_L, KP_5,
    // BackSpace) as iced reports those keys, then a few more of the mapping.
    let presses = [
        (char_key("q"), Location::Standard, "Q"),
        (char_key("Q"), Location::Standard, "Q"),
        (char_key("7"), Location::Standard, "7"),
        (Key::Named(Named::F12), Location::Standard, "F12"),
        (Key::Named(Named::Space), Location::Standard, "Space"),
        (Key::Named(Named::Shift), Location::Left, "Left Shift"),
        (Key::Named(Named::Control), Location::Left, "Left Ctrl"),
        (char_key("5"), Location::Numpad, "Keypad 5"),
        (
            Key::Named(Named::Backspace),
            Location::Standard,
            "Backspace",
        ),
        (Key::Named(Named::Enter), Location::Standard, "Return"),
        (Key::Named(Named::Enter), Location::Numpad, "Keypad Enter"),
        (Key::Named(Named::Alt), Location::Right, "Right Alt"),
        (Key::Named(Named::Escape), Location::Standard, "Escape"),
        (Key::Named(Named::ArrowLeft), Location::Standard, "Left"),
        (
            Key::Named(Named::ContextMenu),
            Location::Standard,
            "Application",
        ),
        (Key::Named(Named::NumLock), Location::Numpad, "Numlock"),
        // The numpad with Num Lock off reports its other names; SDL sees the same keys.
        (Key::Named(Named::Home), Location::Numpad, "Keypad 7"),
        (Key::Named(Named::Delete), Location::Numpad, "Keypad ."),
        (char_key("="), Location::Standard, "Equals"),
        (char_key(","), Location::Standard, "Comma"),
        (char_key("-"), Location::Standard, "-"),
        (Key::Named(Named::F24), Location::Standard, "F24"),
    ];
    for (key, location, name) in presses {
        capture(&mut state, "cross");
        assert_eq!(state.controls.capturing.as_deref(), Some("cross"));
        press_key(&mut state, key.clone(), location);
        assert_eq!(state.controls.key["cross"], name, "{key:?} {location:?}");
        assert!(state.controls.capturing.is_none(), "{key:?}");
    }
    // While no box waits, the keys navigate as before.
    press_key(&mut state, Key::Named(Named::ArrowLeft), Location::Standard);
    assert_eq!(state.page, Page::Trophies);
}

#[test]
fn keys_that_cannot_be_bound() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    let before = state.controls.key.clone();
    // Insert, Win_L, Win_R, Print, F25 and eacute of the Python test.
    let presses = [
        (Key::Named(Named::Insert), Location::Standard),
        (Key::Named(Named::Super), Location::Left),
        (Key::Named(Named::Super), Location::Right),
        (Key::Named(Named::PrintScreen), Location::Standard),
        (Key::Named(Named::F25), Location::Standard),
        (char_key("é"), Location::Standard),
    ];
    capture(&mut state, "cross");
    for (key, location) in presses {
        press_key(&mut state, key.clone(), location);
        // The box keeps waiting and nothing changed.
        assert_eq!(
            state.controls.capturing.as_deref(),
            Some("cross"),
            "{key:?}"
        );
        assert_eq!(state.controls.key, before, "{key:?}");
    }
    assert_eq!(state.page, Page::Controls);
}

#[test]
fn binding_a_used_key_swaps_the_buttons_in_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    // Nobody has F by default: Square takes it.
    capture(&mut state, "square");
    press_key(&mut state, char_key("f"), Location::Standard);
    assert_eq!(state.controls.key["square"], "F");
    // Cross's box shows Space; clicking it waits for an input for Cross.
    click_tall(&mut state, "Space");
    assert_eq!(state.controls.capturing.as_deref(), Some("cross"));
    assert!(tall_has(&state, "Press a key or mouse (5)"));
    press_key(&mut state, char_key("f"), Location::Standard);
    assert_eq!(state.controls.key["cross"], "F");
    assert_eq!(state.controls.key["square"], "Space");
    // The rendered boxes swapped too: F is Cross's box, Space is Square's.
    assert!(matches!(
        tall_messages(&state, "F").as_slice(),
        [Message::Controls(ControlsMessage::Capture(button))] if button == "cross"
    ));
    assert!(matches!(
        tall_messages(&state, "Space").as_slice(),
        [Message::Controls(ControlsMessage::Capture(button))] if button == "square"
    ));
    let rows = controls::key_rows(&state, "en");
    let input = |button: &str| {
        rows.iter()
            .find(|row| row.button == button)
            .unwrap()
            .input
            .clone()
    };
    assert_eq!(
        (input("cross"), input("square")),
        ("F".into(), "Space".into())
    );
}

#[test]
fn device_is_its_own_choice() {
    assert_eq!(Device::ALL.map(Device::value), ["keyboard", "gamepad"]);
    let styles: Vec<&str> = PAD_STYLES.iter().map(|(style, _)| *style).collect();
    assert_eq!(styles, ["playstation", "xbox", "nintendo"]);

    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    // The first open looks for a gamepad once.
    assert!(state.controls.gamepad_probe_started);
    assert_eq!(state.controls.device, Device::Keyboard);
    assert!(!state.controls.device_chosen);
    // A gamepad found shows its table, but an untouched first choice stays automatic.
    controls_update(&mut state, ControlsMessage::GamepadDetected(true));
    assert_eq!(state.controls.device, Device::Gamepad);
    assert!(!state.controls.device_chosen);
    assert!(state.settings.get("controls_device").is_none());
    assert!(tall_has(&state, "GAMEPAD MAPPING"));
    assert!(!tall_has(&state, "KEYBOARD & MOUSE MAPPING"));

    // The device radio only chooses the table shown; the tables keep their inputs.
    capture(&mut state, "cross");
    press_key(&mut state, char_key("g"), Location::Standard);
    let (key, pad) = (state.controls.key.clone(), state.controls.pad.clone());
    click(&mut state, "Keyboard & mouse");
    assert_eq!(state.controls.device, Device::Keyboard);
    assert!(state.controls.device_chosen);
    assert_eq!(state.settings.get_str("controls_device"), "keyboard");
    assert!(tall_has(&state, "KEYBOARD & MOUSE MAPPING"));
    assert!(!tall_has(&state, "GAMEPAD MAPPING"));
    assert_eq!((&state.controls.key, &state.controls.pad), (&key, &pad));
    // Once chosen, a gamepad found later does not switch the page.
    controls_update(&mut state, ControlsMessage::GamepadDetected(true));
    assert_eq!(state.controls.device, Device::Keyboard);
    click(&mut state, "Gamepad");
    assert_eq!(state.controls.device, Device::Gamepad);
    assert_eq!(state.settings.get_str("controls_device"), "gamepad");
    assert_eq!((&state.controls.key, &state.controls.pad), (&key, &pad));
    // The saved choice comes back and the probe does not run.
    let again = ControlsView::new(&state.settings);
    assert_eq!(again.device, Device::Gamepad);
    assert!(again.device_chosen);
    let mut reopened = state;
    reopened.controls = again;
    open(&mut reopened, Page::Controls);
    assert!(!reopened.controls.gamepad_probe_started);
}

#[test]
fn nintendo_names_follow_the_button_positions() {
    let names = controls::pad_labels("nintendo", "en");
    let label = |sdl: &str| {
        names
            .iter()
            .find(|(name, _)| name == sdl)
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(
        (label("a"), label("b"), label("x"), label("y")),
        ("B".into(), "A".into(), "Y".into(), "X".into())
    );
    // A label maps back to one button.
    let unique: std::collections::BTreeSet<&String> = names.iter().map(|(_, l)| l).collect();
    assert_eq!(unique.len(), names.len());

    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    click(&mut state, "Gamepad");
    assert_eq!(state.controls.pad_style, "playstation");
    click_tall(&mut state, "Nintendo");
    assert_eq!(state.controls.pad_style, "nintendo");
    assert_eq!(state.settings.get_str("pad_style"), "nintendo");
    // The table relabels its boxes; L2, R2 and Touchpad left keep their fixed text.
    let rows = controls::pad_rows(&state, "en");
    let row = |button: &str| {
        rows.iter()
            .find(|row| row.button == button)
            .unwrap()
            .clone()
    };
    let PadChoice::Pick { pairs, current } = row("cross").choice else {
        panic!("cross has a box");
    };
    assert_eq!(current, "B");
    assert_eq!(pairs, controls::pad_labels("nintendo", "en"));
    assert!(matches!(row("circle").choice, PadChoice::Pick { current, .. } if current == "A"));
    assert!(matches!(row("l2").choice, PadChoice::Fixed(text) if text == "L2 trigger"));
    assert!(
        matches!(row("touchpad_left").choice, PadChoice::Fixed(text) if text == "L1 + Touchpad right")
    );
    page_has(&state, &["L2 trigger", "R2 trigger", "L1 + Touchpad right"]);
    // Choosing a label stores its SDL name: Nintendo's A is the east button.
    let pick = controls::pad_pick("cross", &pairs, "A");
    assert!(matches!(
        &pick,
        Message::Controls(ControlsMessage::PadPick(button, name)) if button == "cross" && name == "b"
    ));
    let _ = app::update(&mut state, pick);
    assert_eq!(state.controls.pad["cross"], "b");
    assert_eq!(state.controls.pad["circle"], "b");
}

#[test]
fn capture_takes_the_mouse_and_gives_up_in_time() {
    let pressed = |button| iced::Event::Mouse(mouse::Event::ButtonPressed(button));
    let wheel = |delta| iced::Event::Mouse(mouse::Event::WheelScrolled { delta });
    let bind = MouseInput::Bind;
    let inputs = [
        (pressed(mouse::Button::Left), Some(bind("Mouse Left"))),
        (pressed(mouse::Button::Middle), Some(bind("Mouse Middle"))),
        (pressed(mouse::Button::Right), Some(bind("Mouse Right"))),
        (pressed(mouse::Button::Back), Some(bind("Mouse Back"))),
        (pressed(mouse::Button::Forward), Some(bind("Mouse Forward"))),
        // A button without an input name: the box gives up (Python `clicked` -> `finish`).
        (pressed(mouse::Button::Other(8)), Some(MouseInput::GiveUp)),
        (
            wheel(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            Some(bind("Wheel Up")),
        ),
        (
            wheel(mouse::ScrollDelta::Pixels { x: 0.0, y: -40.0 }),
            Some(bind("Wheel Down")),
        ),
        (
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            None,
        ),
    ];
    for (event, name) in inputs {
        assert_eq!(controls::mouse_input(&event), name, "{event:?}");
    }

    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    capture(&mut state, "cross");
    controls_update(
        &mut state,
        ControlsMessage::Captured("cross".into(), "Wheel Up".into()),
    );
    assert_eq!(state.controls.key["cross"], "Wheel Up");
    assert!(state.controls.capturing.is_none());
    // An input that arrives when no box waits binds nothing.
    controls_update(
        &mut state,
        ControlsMessage::Captured("cross".into(), "Mouse Left".into()),
    );
    assert_eq!(state.controls.key["cross"], "Wheel Up");

    // The box counts down and gives up after CAPTURE_SECONDS.
    assert_eq!(CAPTURE_SECONDS, 5);
    capture(&mut state, "circle");
    let started = state.controls.capture_started.unwrap();
    let _ = app::update(
        &mut state,
        Message::Tick(started + std::time::Duration::from_millis(4900)),
    );
    assert_eq!(state.controls.capturing.as_deref(), Some("circle"));
    assert!(tall_has(&state, "Press a key or mouse (1)"));
    // While it waits, a click anywhere is only its input (the mouse subscription delivers it):
    // the button, the box or the tab under the cursor does not react.
    for label in ["Reset keyboard & mouse to defaults", "Wheel Up", "Graphics"] {
        let sent = tall_messages(&state, label);
        assert!(
            sent.iter().all(|message| matches!(message, Message::Noop)),
            "{label}: {sent:?}"
        );
    }
    let _ = app::update(
        &mut state,
        Message::Tick(started + std::time::Duration::from_secs(CAPTURE_SECONDS)),
    );
    assert!(state.controls.capturing.is_none());
    assert_eq!(state.controls.key["circle"], "Left Shift");
    assert!(tall_has(&state, "Left Shift"));
}

#[test]
fn reset_buttons_and_the_rage_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    let defaults = state.controls.key.clone();
    assert_eq!(defaults["square"], "Mouse Left");
    assert_eq!(defaults["rage"], "Mouse Middle");
    // The arrow puts one button back on its default, swapping with the button that had it.
    capture(&mut state, "cross");
    press_key(&mut state, char_key("h"), Location::Standard);
    assert_eq!(state.controls.key["cross"], "H");
    controls_update(&mut state, ControlsMessage::ResetKey("cross".into()));
    assert_eq!(state.controls.key["cross"], "Space");
    // L1 leaves Q, R1 takes it: L1's arrow gives L1 its Q back and R1 the old H.
    capture(&mut state, "l1");
    press_key(&mut state, char_key("h"), Location::Standard);
    capture(&mut state, "r1");
    press_key(&mut state, char_key("q"), Location::Standard);
    controls_update(&mut state, ControlsMessage::ResetKey("l1".into()));
    assert_eq!(state.controls.key["l1"], "Q");
    assert_eq!(state.controls.key["r1"], "H");
    click_tall(&mut state, "Reset keyboard & mouse to defaults");
    assert_eq!(state.controls.key, defaults);

    // Rage of Sparta on one input greys L3 and R3; off, it greys the Rage row.
    let enabled = |state: &app::State, button: &str| {
        controls::key_rows(state, "en")
            .into_iter()
            .find(|row| row.button == button)
            .unwrap()
            .enabled
    };
    assert!(state.settings.get_bool("rage_one_input"));
    assert!(!enabled(&state, "l3") && !enabled(&state, "r3") && enabled(&state, "rage"));
    click_tall(&mut state, "Rage of Sparta on one input (L3 + R3 together)");
    assert!(!state.settings.get_bool("rage_one_input"));
    assert!(enabled(&state, "l3") && enabled(&state, "r3") && !enabled(&state, "rage"));
    assert!(enabled(&state, "cross"));

    // The gamepad table goes back to the controller's own buttons.
    controls_update(
        &mut state,
        ControlsMessage::PadPick("cross".into(), "y".into()),
    );
    controls_update(&mut state, ControlsMessage::ResetTable(Kind::Pad));
    assert_eq!(state.controls.pad["cross"], "a");
    assert_eq!(state.controls.pad["l2"], "");
}

#[test]
fn a_gamepad_found_while_a_box_waits_ends_the_capture() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    let before = state.controls.key.clone();
    // The player clicks a key box before the first-open probe answers.
    capture(&mut state, "cross");
    controls_update(&mut state, ControlsMessage::GamepadDetected(true));
    assert_eq!(state.controls.device, Device::Gamepad);
    assert!(state.controls.capturing.is_none());
    assert!(state.controls.capture_started.is_none());
    assert_eq!(state.controls.key, before);
    // No hidden box takes the next click: it reaches the gamepad section.
    assert!(matches!(
        tall_messages(&state, "Nintendo").as_slice(),
        [Message::Controls(ControlsMessage::PadStyle(style))] if style == "nintendo"
    ));
}

#[test]
fn losing_the_focus_ends_the_capture() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = controls_state(dir.path());
    let before = state.controls.key.clone();
    capture(&mut state, "cross");
    let _ = app::update(&mut state, Message::WindowFocused(false));
    assert!(!state.focused);
    assert!(state.controls.capturing.is_none());
    assert!(state.controls.capture_started.is_none());
    assert_eq!(state.controls.key, before);
    // The next key navigates again instead of binding.
    let _ = app::update(&mut state, Message::WindowFocused(true));
    press_key(&mut state, Key::Named(Named::ArrowLeft), Location::Standard);
    assert_eq!(state.page, Page::Trophies);
    assert_eq!(state.controls.key, before);
}

// --- Mods page (Task 29) ---

use spartanport_launcher::ui::pages::mods::ModsMessage;

fn mods_state(dir: &std::path::Path) -> app::State {
    let mut state = state(dir);
    state.lang = "en".into();
    state
}

fn mods_update(state: &mut app::State, message: ModsMessage) {
    let _ = app::update(state, Message::Mods(message));
}

#[test]
fn mods_list_follows_the_folder_and_saved_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    for name in ["a", "b", "c"] {
        make_mod(&state, name, &format!("{name}.bin"));
    }
    std::fs::write(
        state.paths.mods_json(),
        r#"{"order": ["b","a"], "disabled": ["a"]}"#,
    )
    .unwrap();
    load_mods(&mut state);
    assert_eq!(state.mods.order, ["b", "a", "c"]);
    assert!(state.mods.enabled["b"]);
    assert!(!state.mods.enabled["a"]);
    assert!(state.mods.enabled["c"]);
    open(&mut state, Page::Mods);
    assert!(tall_has(&state, "2 of 3 active"));
}

#[test]
fn moving_a_mod_changes_the_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    for name in ["a", "b", "c"] {
        make_mod(&state, name, &format!("{name}.bin"));
    }
    load_mods(&mut state);
    assert_eq!(state.mods.order, ["a", "b", "c"]);
    mods_update(&mut state, ModsMessage::Move(0, 1));
    assert_eq!(state.mods.order, ["b", "a", "c"]);
    // Past either end nothing moves.
    mods_update(&mut state, ModsMessage::Move(0, -1));
    mods_update(&mut state, ModsMessage::Move(2, 1));
    assert_eq!(state.mods.order, ["b", "a", "c"]);
}

#[test]
fn conflict_summary_names_the_winner() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    make_mod(&state, "alpha", "same.bin");
    make_mod(&state, "beta", "same.bin");
    load_mods(&mut state);
    let found = spartanport_launcher::core::mods::conflicts(
        &state.paths.mods_dir(""),
        &state.mods.order,
        None,
    );
    let names = state.mods.order.clone();
    mods_update(&mut state, ModsMessage::Conflicts(names, found));
    open(&mut state, Page::Mods);
    assert!(tall_has(
        &state,
        "⚠ 1 file is changed by two mods. beta wins."
    ));
    // The lower mod moves up: the other one wins.
    mods_update(&mut state, ModsMessage::Move(1, -1));
    assert_eq!(state.mods.order, ["beta", "alpha"]);
    let found = spartanport_launcher::core::mods::conflicts(
        &state.paths.mods_dir(""),
        &state.mods.order,
        None,
    );
    let names = state.mods.order.clone();
    mods_update(&mut state, ModsMessage::Conflicts(names, found));
    assert!(tall_has(
        &state,
        "⚠ 1 file is changed by two mods. alpha wins."
    ));
}

#[test]
fn preset_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    for name in ["a", "b"] {
        make_mod(&state, name, &format!("{name}.bin"));
    }
    load_mods(&mut state);
    // No name: an Info dialog and no file.
    mods_update(&mut state, ModsMessage::SavePreset);
    let modal = state.modal.take().unwrap();
    assert_eq!(modal.text, "Type a name for the preset first.");
    assert!(!state.paths.mod_presets().exists());

    mods_update(&mut state, ModsMessage::Toggle("a".into()));
    mods_update(&mut state, ModsMessage::Move(0, 1));
    mods_update(&mut state, ModsMessage::PresetName("x".into()));
    mods_update(&mut state, ModsMessage::SavePreset);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state.paths.mod_presets()).unwrap()).unwrap();
    assert_eq!(
        saved,
        serde_json::json!({"x": {"order": ["b", "a"], "disabled": ["a"]}})
    );
    assert_eq!(state.mods.presets, ["x"]);

    mods_update(&mut state, ModsMessage::Toggle("a".into()));
    mods_update(&mut state, ModsMessage::Move(0, 1));
    assert_eq!(state.mods.order, ["a", "b"]);
    assert!(state.mods.enabled["a"]);
    mods_update(&mut state, ModsMessage::LoadPreset);
    assert_eq!(state.mods.order, ["b", "a"]);
    assert!(!state.mods.enabled["a"]);

    mods_update(&mut state, ModsMessage::DeletePreset);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state.paths.mod_presets()).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({}));
    assert!(state.mods.presets.is_empty());
    assert!(state.mods.preset_name.is_empty());
}

#[test]
fn the_list_follows_the_mods() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    performance::refresh_mod_resolutions(&mut state);
    let builtin = performance::resolution_options(&state).len();
    // No mod yet.
    load_mods(&mut state);
    assert_eq!(performance::resolution_options(&state).len(), builtin);
    // The Mods page lists the new folder.
    let patches = state.paths.mods_dir("").join("Ultrawide").join("patches");
    std::fs::create_dir_all(&patches).unwrap();
    write_mod_patch(&patches, "3840x1080");
    load_mods(&mut state);
    let options = performance::resolution_options(&state);
    assert_eq!(options.len(), builtin + 1);
    let label = options.iter().find(|(v, _)| v == "3840x1080").unwrap();
    assert_eq!(label.1, "3840 × 1080 · 32:9 (mod)");
    // Unticked on the Mods page.
    mods_update(&mut state, ModsMessage::Toggle("Ultrawide".into()));
    assert_eq!(performance::resolution_options(&state).len(), builtin);
    mods_update(&mut state, ModsMessage::Toggle("Ultrawide".into()));
    assert_eq!(performance::resolution_options(&state).len(), builtin + 1);
    // "Load mods" off.
    let _ = app::update(
        &mut state,
        Message::Set("mods_enabled".into(), false.into()),
    );
    assert_eq!(performance::resolution_options(&state).len(), builtin);
}

// --- Advanced page (Task 30) ---

use spartanport_launcher::core::options::{UI_LANGUAGES, UI_SCALES};
use spartanport_launcher::ui::pages::advanced;
use spartanport_launcher::ui::play::GameState;

fn pick_row(state: &app::State, key: &str, value: &str) -> Message {
    let rows = advanced::rows(state, "en");
    let row = rows.iter().find(|row| row.key == key).unwrap();
    let label = &row.pairs.iter().find(|(v, _)| v == value).unwrap().1;
    let choices = match key {
        "ui_language" => UI_LANGUAGES,
        _ => UI_SCALES,
    };
    select_message(key, choices, "en", label).unwrap()
}

#[test]
fn language_and_scale_rows_change_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    let pick = pick_row(&state, "ui_language", "pt");
    let _ = app::update(&mut state, pick);
    assert_eq!(state.settings.get_str("ui_language"), "pt");
    // The interface changes language when the launcher opens again (Save is Task 32).
    assert_eq!(state.lang, "en");
    let pick = pick_row(&state, "ui_scale", "2");
    let _ = app::update(&mut state, pick);
    assert_eq!(state.settings.get_str("ui_scale"), "2");
}

#[test]
fn clear_cache_is_blocked_while_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    // An idle game does not block it; `ui_play.rs` covers a running one.
    assert!(!advanced::clear_cache_blocked(&state));
    state.game = GameState::Idle;
    let cache = dir.path().join("user").join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("a.bin"), b"x").unwrap();
    assert_eq!(advanced::clear_cache(&dir.path().join("user")), Ok(()));
    assert!(!cache.exists());
    // Nothing to remove is not an error.
    assert_eq!(advanced::clear_cache(&dir.path().join("user")), Ok(()));
    let _ = app::update(&mut state, Message::ShaderCacheCleared(Ok(())));
    let shown = state.modal.take().unwrap();
    assert!(matches!(shown.kind, ModalKind::Info));
    assert!(shown.text.starts_with("Shader cache cleared."));
    let _ = app::update(&mut state, Message::ShaderCacheCleared(Err("busy".into())));
    let shown = state.modal.take().unwrap();
    assert!(matches!(shown.kind, ModalKind::Error));
    assert_eq!(shown.text, "busy");
}

#[test]
fn effects_row_toggles_the_fire() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    assert!(state.effects);
    open(&mut state, Page::Advanced);
    click(&mut state, "Effects");
    assert_eq!(state.settings.get("launcher_effects"), Some(&false.into()));
    assert!(!state.effects);
    click(&mut state, "Effects");
    assert!(state.effects);
    // Saved off: the next start begins with the fire off.
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"launcher_effects": false}"#,
    )
    .unwrap();
    let again = common::state(dir.path());
    assert!(!again.effects);
}

#[test]
fn check_updates_row_hidden_without_self_update() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.self_update = false;
    assert!(!advanced::check_keys(&state).contains(&"check_updates"));
    assert!(advanced::check_keys(&state).contains(&"close_on_play"));
    state.self_update = true;
    assert!(advanced::check_keys(&state).contains(&"check_updates"));
}

#[test]
fn diagnostics_start_collapsed() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    assert!(!advanced::check_keys(&state).contains(&"frame_stats"));
    let _ = app::update(&mut state, Message::ToggleDiagnostics);
    assert!(advanced::check_keys(&state).contains(&"vk_validation"));
}

#[test]
fn steam_bridge_command_matches_the_spec() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.settings.set("game_dir", "D:/CUSA01623".into());
    let exe = std::path::Path::new("C:/SP/spartanport-launcher.exe");
    let command = advanced::steam_bridge_command(&state.paths, &state.settings, exe);
    let port = state.paths.port_dir.display();
    assert_eq!(command[0], "-c");
    assert_eq!(
        command[1],
        format!(
            "import sys; sys.path.insert(0, r'{port}/scripts'); import steam_shortcut as s; \
             print(s.add_to_steam(r'D:/CUSA01623', (r'C:/SP/spartanport-launcher.exe', '--play', r'{port}'), r'{port}/launcher/spartanport.png'))"
        )
        .replace(
            "/launcher/spartanport.png",
            &format!("{}launcher{}spartanport.png", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR)
        )
    );
    assert_eq!(command.len(), 2);
}

#[test]
fn steam_answers_become_the_python_texts() {
    let added = advanced::steam_outcome(
        "en",
        true,
        "{'accounts': 2, 'updated': False, 'app_id': 1}",
        "",
    );
    assert!(added
        .unwrap()
        .starts_with("The shortcut \"SpartanPort\" was added to Steam (accounts: 2)"));
    let updated = advanced::steam_outcome("en", true, "{'accounts': 1, 'updated': True}", "");
    assert!(updated
        .unwrap()
        .contains("was updated in Steam (accounts: 1)"));
    let running = advanced::steam_outcome(
        "en",
        false,
        "",
        "Traceback\nsteam_shortcut.SteamError: running\n",
    );
    assert!(running.unwrap_err().starts_with("Steam is open."));
    let none = advanced::steam_outcome("en", false, "", "steam_shortcut.SteamError: no_steam");
    assert_eq!(none.unwrap_err(), "Steam was not found for this user.");
    let art = advanced::steam_outcome("en", false, "", "steam_shortcut.SteamError: no_art: x");
    assert!(art
        .unwrap_err()
        .starts_with("The game folder has no sce_sys/pic1.png"));
    let other = advanced::steam_outcome("en", false, "", "Traceback\nImportError: nope\n");
    assert_eq!(other.unwrap_err(), "ImportError: nope");
}

#[test]
fn stale_conflicts_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = mods_state(dir.path());
    make_mod(&state, "alpha", "same.bin");
    make_mod(&state, "beta", "same.bin");
    load_mods(&mut state);
    let root = state.paths.mods_dir("");
    let old_names = state.mods.order.clone();
    let old = spartanport_launcher::core::mods::conflicts(&root, &old_names, None);
    assert_eq!(old[0].1.last().unwrap(), "beta");
    // An edit lands before the older result does.
    mods_update(&mut state, ModsMessage::Move(1, -1));
    let new_names = state.mods.order.clone();
    assert_eq!(new_names, ["beta", "alpha"]);
    mods_update(&mut state, ModsMessage::Conflicts(old_names, Vec::new()));
    assert_eq!(state.mods.conflicts, old);
    let current = spartanport_launcher::core::mods::conflicts(&root, &new_names, None);
    assert_eq!(current[0].1.last().unwrap(), "alpha");
    // The stale result changed nothing; the matching one applies.
    mods_update(
        &mut state,
        ModsMessage::Conflicts(new_names.clone(), current.clone()),
    );
    assert_eq!(state.mods.conflicts, current);
    // And a stale one after it is still dropped.
    mods_update(
        &mut state,
        ModsMessage::Conflicts(vec!["alpha".into(), "beta".into()], old),
    );
    assert_eq!(state.mods.conflicts, current);
}

// --- Log page (Task 31) ---

use spartanport_launcher::ui::pages::log::{self, LogMessage};

#[test]
fn log_page_shows_buffer_lines_and_clears() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "en".into();
    state.log.push_text("line one\nline two\n");
    open(&mut state, Page::Log);
    let mut ui = iced_test::simulator::simulator(app::view(&state));
    // The body is one text widget, so it is found by its whole text.
    assert!(ui
        .find(
            "line one
line two"
        )
        .is_ok());
    drop(ui);

    // Copy hands the text to the clipboard; the buffer stays as it was.
    click(&mut state, "Copy");
    assert_eq!(state.log.len(), 2);

    // Open last_run.log targets <user_dir>/last_run.log.
    let expected = state
        .paths
        .run_log_path(&state.settings.get_str("user_dir"));
    assert!(matches!(
        log::open_last_run_message(&state),
        Message::OpenPath(path) if path == expected
    ));
    let sent = messages(&state, "Open last_run.log");
    assert!(matches!(
        sent.as_slice(),
        [Message::Log(LogMessage::OpenLastRun)]
    ));
    let _ = app::update(&mut state, Message::Log(LogMessage::OpenLastRun));

    click(&mut state, "Clear");
    assert_eq!(state.log.len(), 0);
    let mut ui = iced_test::simulator::simulator(app::view(&state));
    assert!(ui
        .find(
            "line one
line two"
        )
        .is_err());
}

#[test]
fn log_header_translates() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    state.lang = "pt".into();
    open(&mut state, Page::Log);
    let mut ui = iced_test::simulator::simulator(app::view(&state));
    assert!(ui.find("6 000 linhas mantidas").is_ok());
}

/// The update offer and install (Task 34): nothing here reaches the network or spawns anything.
mod update_flow {
    use std::path::{Path, PathBuf};

    use iced_test::simulator::simulator;
    use spartanport_launcher::core::update::Release;
    use spartanport_launcher::ui::app::{self, Message};
    use spartanport_launcher::ui::splash;
    use spartanport_launcher::ui::update::{
        install_decision, offer_decision, Decision, InstallPlan, UpdateMessage, UpdateState,
    };
    use spartanport_launcher::ui::widgets::ModalKind;
    use spartanport_launcher::VERSION;

    use super::common::state;

    fn release(version: &str, url: Option<&str>) -> Release {
        Release {
            version: version.to_string(),
            url: url.map(str::to_string),
            page: "https://example.test/page".to_string(),
        }
    }

    fn checked(manual: bool, result: Result<Release, String>) -> Message {
        Message::Update(UpdateMessage::Checked { manual, result })
    }

    fn modal_text(state: &app::State) -> String {
        state
            .modal
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default()
    }

    #[test]
    fn a_newer_release_shows_the_pill() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        let _ = app::update(
            &mut state,
            checked(
                false,
                Ok(release("999.0.0", Some("https://example.test/a.zip"))),
            ),
        );
        assert!(matches!(&state.update, UpdateState::Available(r) if r.version == "999.0.0"));
        assert_eq!(state.status, "Version 999.0.0 is available.");
        assert!(state.modal.is_none());
        assert!(simulator(app::view(&state))
            .find("Version 999.0.0 is available · Update")
            .is_ok());
    }

    #[test]
    fn manual_check_with_latest_shows_the_info() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        let _ = app::update(&mut state, checked(true, Ok(release(VERSION, None))));
        assert!(matches!(state.update, UpdateState::None));
        assert_eq!(
            modal_text(&state),
            format!("You have the latest version ({VERSION}).")
        );
        assert!(matches!(
            state.modal.as_ref().map(|m| &m.kind),
            Some(ModalKind::Info)
        ));
        // The same release found silently shows nothing.
        state.modal = None;
        let _ = app::update(&mut state, checked(false, Ok(release(VERSION, None))));
        assert!(state.modal.is_none());
    }

    #[test]
    fn a_failed_manual_check_reports_and_a_silent_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        let _ = app::update(&mut state, checked(true, Err("offline".into())));
        assert_eq!(modal_text(&state), "Could not check for updates: offline");
        state.modal = None;
        let _ = app::update(&mut state, checked(false, Err("offline".into())));
        assert!(state.modal.is_none());
        assert!(matches!(state.update, UpdateState::None));
    }

    #[test]
    fn the_offer_depends_on_the_build_and_the_system() {
        let newer = release("999.0.0", None);
        assert_eq!(offer_decision(&newer, false, false), Decision::Nothing);
        assert_eq!(offer_decision(&newer, true, false), Decision::NoLinuxBuild);
        assert_eq!(offer_decision(&newer, false, true), Decision::Offer);
        let built = release("999.0.0", Some("https://example.test/a.AppImage"));
        assert_eq!(offer_decision(&built, false, false), Decision::Offer);
        let same = release(VERSION, Some("https://example.test/a.zip"));
        assert_eq!(offer_decision(&same, true, true), Decision::Latest);
        assert_eq!(offer_decision(&same, false, true), Decision::Nothing);

        // The message itself only shows on a host without a Windows build.
        if !cfg!(windows) {
            let dir = tempfile::tempdir().unwrap();
            let mut state = state(dir.path());
            let _ = app::update(&mut state, checked(true, Ok(newer)));
            assert_eq!(
                modal_text(&state),
                "Version 999.0.0 is out, but it has no Linux build yet."
            );
        }
    }

    #[test]
    fn the_start_up_offer_follows_the_url_or_windows_rule() {
        let no_url = release("999.0.0", None);
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        splash::start_up_offer(&mut state, no_url.clone(), false);
        assert!(matches!(state.update, UpdateState::None));
        assert!(state.status.is_empty());
        splash::start_up_offer(&mut state, no_url, true);
        assert!(matches!(&state.update, UpdateState::Available(_)));
        // A release with a package is offered on both systems.
        let built = release("999.0.0", Some("https://example.test/a.AppImage"));
        let mut other = super::common::state(dir.path());
        splash::start_up_offer(&mut other, built, false);
        assert!(matches!(&other.update, UpdateState::Available(_)));
    }

    #[test]
    fn install_is_blocked_while_running() {
        let built = release("999.0.0", Some("https://example.test/a.zip"));
        let always = |_: &Path| true;
        assert_eq!(
            install_decision(&built, true, true, true, None, &always),
            InstallPlan::Blocked
        );
        assert_eq!(
            install_decision(&built, false, true, true, None, &always),
            InstallPlan::Confirm
        );
    }

    #[test]
    fn windows_source_tree_opens_the_page() {
        let built = release("999.0.0", Some("https://example.test/a.zip"));
        let always = |_: &Path| true;
        assert_eq!(
            install_decision(&built, false, true, false, None, &always),
            InstallPlan::OpenPage
        );
        let no_url = release("999.0.0", None);
        assert_eq!(
            install_decision(&no_url, false, true, true, None, &always),
            InstallPlan::OpenPage
        );
    }

    #[test]
    fn linux_needs_a_writable_appimage_folder() {
        let built = release("999.0.0", Some("https://example.test/a.AppImage"));
        let image = PathBuf::from("/opt/app/SpartanPort.AppImage");
        let yes = |_: &Path| true;
        let no = |_: &Path| false;
        assert_eq!(
            install_decision(&built, false, false, false, None, &yes),
            InstallPlan::OpenPage
        );
        assert_eq!(
            install_decision(&built, false, false, false, Some(&image), &yes),
            InstallPlan::Confirm
        );
        assert!(matches!(
            install_decision(&built, false, false, false, Some(&image), &no),
            InstallPlan::ReadOnly(_)
        ));
    }

    #[test]
    fn progress_updates_the_pill_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        let built = release("2.7.0", Some("https://example.test/a.zip"));
        state.update = UpdateState::Downloading(built.clone(), 0);
        let _ = app::update(&mut state, Message::Update(UpdateMessage::Progress(42)));
        assert!(simulator(app::view(&state))
            .find("Downloading version 2.7.0… 42%")
            .is_ok());

        let _ = app::update(
            &mut state,
            Message::Update(UpdateMessage::Done(Err("boom".into()))),
        );
        assert!(matches!(&state.update, UpdateState::Available(r) if *r == built));
        assert_eq!(modal_text(&state), "Update failed: boom");
        assert!(simulator(app::view(&state))
            .find("Version 2.7.0 is available · Update")
            .is_ok());
    }
}
