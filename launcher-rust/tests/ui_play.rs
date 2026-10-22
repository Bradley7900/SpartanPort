#![allow(clippy::unwrap_used, clippy::expect_used)]
//! PLAY and STOP (Task 33): the checks before the start, the live log, the exit and
//! `close_on_play`. Ports of `tests/test_launcher_close_on_play.py` and
//! `tests/test_launcher_run_log.py` (the halves that are not Tk). The game is always the
//! `fake-game` test binary, never `run.py`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced_test::simulator::simulator;
use serde_json::json;
use spartanport_launcher::core::log::ExitHint;
use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::home;
use spartanport_launcher::ui::pages::trophies::TrophiesMessage;
use spartanport_launcher::ui::pages::{advanced, game, Page};
use spartanport_launcher::ui::play::{self, GameMessage, GameState};
use spartanport_launcher::ui::widgets::modal::{ModalKind, ModalMessage};

/// The longest any test waits for the fake game.
const BOUND: Duration = Duration::from_secs(10);

const PREPARING: &str = "Preparing the game; it opens in its own window…";

/// A launcher over `dir` only (its port and data folder), with the splash already gone.
fn state_in(dir: &Path) -> State {
    let root = dir.display().to_string();
    let env = move |key: &str| match key {
        "APPDATA" | "USERPROFILE" | "HOME" | "XDG_CONFIG_HOME" => Some(root.clone()),
        _ => None,
    };
    let mut state = State::new(Paths::from_env(dir, &env));
    state.splash.done = true;
    state
}

fn send(state: &mut State, message: Message) {
    let _ = app::update(state, message);
}

fn set(state: &mut State, key: &str, value: serde_json::Value) {
    send(state, Message::Set(key.to_string(), value));
}

/// PLAY starts the fake game with `args` instead of `run.py`.
fn fake_game(state: &mut State, args: &[&str]) {
    let mut command = vec![env!("CARGO_BIN_EXE_fake-game").to_string()];
    command.extend(args.iter().map(|arg| arg.to_string()));
    state.launch_command = Some(command);
}

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

fn game_sfo() -> Vec<u8> {
    sfo(&[
        ("TITLE_ID", "CUSA01623"),
        ("TITLE", "God of War III"),
        ("APP_VER", "01.02"),
    ])
}

/// `eboot.bin` and a tiny `param.sfo`: what `game_info` needs, without the modules.
fn eboot_and_sfo(folder: &Path) {
    fs::create_dir_all(folder.join("sce_sys")).unwrap();
    fs::write(folder.join("eboot.bin"), b"elf").unwrap();
    fs::write(folder.join("sce_sys").join("param.sfo"), game_sfo()).unwrap();
}

/// A dump with every file PLAY checks.
fn full_dump(folder: &Path) {
    eboot_and_sfo(folder);
    fs::create_dir_all(folder.join("sce_module")).unwrap();
    fs::write(folder.join("sce_module").join("libc.prx"), b"prx").unwrap();
    fs::write(folder.join("sce_module").join("libSceFios2.prx"), b"prx").unwrap();
}

fn text_of(path: &Path) -> String {
    path.display().to_string()
}

/// A launcher over `dir` whose game folder is a full dump and whose PLAY starts the fake game.
fn ready(dir: &Path, args: &[&str]) -> State {
    let dump = dir.join("dump");
    full_dump(&dump);
    let mut state = state_in(dir);
    set(&mut state, "game_dir", json!(text_of(&dump)));
    fake_game(&mut state, args);
    state
}

fn idle(state: &State) -> bool {
    matches!(state.game, GameState::Idle)
}

fn pid(state: &State) -> u32 {
    match &state.game {
        GameState::Preparing { handle, .. } | GameState::Running { handle, .. } => handle.pid,
        other => panic!("no game runs: {other:?}"),
    }
}

fn hidden(state: &State) -> bool {
    match &state.game {
        GameState::Preparing { hidden, .. } | GameState::Running { hidden, .. } => *hidden,
        _ => false,
    }
}

/// Applies the game's events as the clock does until `done` holds (at most `bound`); returns the
/// status after each event.
fn pump(state: &mut State, bound: Duration, done: impl Fn(&State) -> bool) -> Vec<String> {
    let started = Instant::now();
    let mut statuses = Vec::new();
    while !done(state) {
        assert!(
            started.elapsed() < bound,
            "not there after {bound:?}: {:?}",
            state.game
        );
        for message in play::drain_events(state) {
            send(state, Message::Game(message));
            statuses.push(state.status.clone());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    statuses
}

fn error_text(state: &State) -> String {
    let shown = state.modal.clone().expect("an error dialog");
    assert!(matches!(shown.kind, ModalKind::Error), "{shown:?}");
    shown.text
}

/// The messages a click on `label` sends; empty for a disabled button.
fn click(state: &State, label: &str) -> Vec<Message> {
    let mut ui = simulator(app::view(state));
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    ui.into_messages().collect()
}

fn shows(state: &State, label: &str) -> bool {
    simulator(app::view(state)).find(label).is_ok()
}

#[test]
fn play_refuses_without_eboot_and_opens_game_page() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let mut state = state_in(dir.path());
    set(&mut state, "game_dir", json!(text_of(&empty)));
    fake_game(&mut state, &["--lines", "1", "--exit", "0"]);
    send(&mut state, Message::Play);
    assert_eq!(
        error_text(&state),
        "Choose the game folder with eboot.bin (CUSA01623)."
    );
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));
    // PLAY saved the settings first.
    let saved = fs::read_to_string(state.paths.config_file()).unwrap();
    assert!(saved.contains("empty"), "{saved}");
}

#[test]
fn play_names_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let dump = dir.path().join("dump");
    eboot_and_sfo(&dump);
    let mut state = state_in(dir.path());
    set(&mut state, "game_dir", json!(text_of(&dump)));
    fake_game(&mut state, &["--lines", "1", "--exit", "0"]);
    send(&mut state, Message::Play);
    assert_eq!(
        error_text(&state),
        "The game dump is incomplete. Missing or empty files:\n\nsce_module/libc.prx\n\
         sce_module/libSceFios2.prx\n\nDump the game again, or copy these files from a complete dump."
    );
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));

    // An empty eboot.bin still counts as a game folder, and is named as empty.
    full_dump(&dump);
    fs::write(dump.join("eboot.bin"), b"").unwrap();
    send(&mut state, Message::Modal(ModalMessage::Dismiss));
    send(&mut state, Message::ShowPage(Page::Home));
    send(&mut state, Message::Play);
    assert!(
        error_text(&state).contains("files:\n\neboot.bin (empty)\n\n"),
        "{}",
        error_text(&state)
    );
    assert_eq!(state.page, Page::Game);
    fs::write(dump.join("eboot.bin"), b"elf").unwrap();

    // The update folder: the game folder itself is refused, then an incomplete one is named.
    set(&mut state, "update_dir", json!(text_of(&dump)));
    send(&mut state, Message::Play);
    assert_eq!(
        error_text(&state),
        "Choose two separate folders for the game and its update, or leave the update folder empty."
    );
    let update = dir.path().join("update");
    fs::create_dir_all(update.join("sce_sys").join("param.sfo")).unwrap();
    fs::write(update.join("eboot.bin"), b"elf").unwrap();
    set(&mut state, "update_dir", json!(text_of(&update)));
    send(&mut state, Message::ShowPage(Page::Home));
    send(&mut state, Message::Play);
    assert_eq!(
        error_text(&state),
        format!(
            "The update folder {} is incomplete. Missing or empty files:\n\n\
             sce_sys/param.sfo (a folder, not a file)\n\nChoose the folder of the update \
             (eboot.bin, sce_sys), or leave it empty.",
            text_of(&update)
        )
    );
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));
}

#[test]
fn describe_missing_labels_every_state() {
    use spartanport_launcher::core::game::FileState;
    let missing = [
        ("a".to_string(), FileState::Missing),
        ("b".to_string(), FileState::Empty),
        ("c".to_string(), FileState::Unreadable),
        ("d".to_string(), FileState::Invalid),
        ("e".to_string(), FileState::Folder),
    ];
    assert_eq!(
        play::describe_missing("en", &missing),
        [
            "a",
            "b (empty)",
            "c (unreadable)",
            "d (not a param.sfo)",
            "e (a folder, not a file)"
        ]
    );
}

#[test]
fn play_runs_the_fake_game_and_shows_its_lines_then_the_exit() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(dir.path(), &["--lines", "5", "--exit", "3"]);
    state.log.push_text("an older run\n");
    assert!(click(&state, "PLAY")
        .iter()
        .any(|message| matches!(message, Message::Play)));

    send(&mut state, Message::Play);
    assert!(
        matches!(state.game, GameState::Preparing { hidden: false, .. }),
        "{:?}",
        state.game
    );
    assert_eq!(state.status, PREPARING);
    assert_eq!(home::chips(&state)[3], PREPARING);
    assert!(state.log.is_empty(), "the log starts empty");
    assert!(state.modal.is_none());
    // PLAY turns into a disabled RUNNING next to STOP.
    assert!(click(&state, "RUNNING").is_empty());
    assert!(click(&state, "STOP")
        .iter()
        .any(|message| matches!(message, Message::Stop)));
    // A second PLAY while it runs does nothing.
    let first = pid(&state);
    send(&mut state, Message::Play);
    assert_eq!(pid(&state), first);

    // What run.py prints moves the status on; a line naming a known failure keeps its advice.
    let line = |text: &str| Message::Game(GameMessage::Output(vec![text.to_string()]));
    send(&mut state, line("Entering original x86-64 code\n"));
    assert!(matches!(state.game, GameState::Running { .. }));
    assert_eq!(state.status, "The game is running.");
    send(&mut state, line("restarting through run.py\n"));
    assert_eq!(state.status, "Restarting with the new settings…");
    send(
        &mut state,
        line("mmap failed of guest memory (error 1455)\n"),
    );
    assert!(matches!(
        state.game,
        GameState::Running {
            hint: Some(ExitHint::GuestMemory),
            ..
        }
    ));

    let statuses = pump(&mut state, BOUND, idle);
    assert!(statuses.contains(&"Restarting with the new settings…".to_string()));
    assert_eq!(statuses.last().map(String::as_str), Some(""));
    let log: Vec<&str> = state.log.lines().collect();
    for k in 1..=5 {
        assert!(log.contains(&format!("line {k}").as_str()), "{log:?}");
    }
    let text = state.log.text();
    assert!(
        text.ends_with(&format!(
            "line 5\n\nThe game exited (code 3).\n{}",
            ExitHint::GuestMemory.text_en()
        )),
        "{text}"
    );
    // PLAY is back, with the summary in the status chip.
    assert!(idle(&state));
    assert!(state.status.is_empty());
    assert_eq!(home::chips(&state)[3], home::summary(&state));
    assert!(!shows(&state, "RUNNING"));
    assert!(!shows(&state, "STOP"));
    assert!(click(&state, "PLAY")
        .iter()
        .any(|message| matches!(message, Message::Play)));
}

#[test]
fn the_clock_drains_the_game_output() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(dir.path(), &["--lines", "2", "--exit", "0"]);
    send(&mut state, Message::Play);
    let started = Instant::now();
    while !idle(&state) {
        assert!(started.elapsed() < BOUND, "{:?}", state.game);
        send(&mut state, Message::Tick(iced::time::Instant::now()));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(state
        .log
        .text()
        .contains("line 2\n\nThe game exited (code 0)."));
}

#[test]
fn stop_terminates_and_reports() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(
        dir.path(),
        &["--lines", "1000", "--delay-ms", "50", "--exit", "0"],
    );
    send(&mut state, Message::Play);
    pump(&mut state, BOUND, |state| !state.log.is_empty());
    assert!(!idle(&state));
    send(&mut state, Message::Stop);
    pump(&mut state, Duration::from_secs(5), idle);
    let text = state.log.text();
    assert!(text.contains("line 1"), "{text}");
    assert!(!text.contains("line 1000"), "{text}");
    assert!(text.contains("\n\nThe game exited (code "), "{text}");
    assert!(!text.contains("(code 0)"), "{text}");
}

#[test]
fn close_on_play_hides_only_while_running() {
    assert_eq!(play::HIDE_DELAY, Duration::from_secs(5));
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(
        dir.path(),
        &["--lines", "1000", "--delay-ms", "50", "--exit", "0"],
    );
    set(&mut state, "close_on_play", json!(true));

    // test_active_game_hides_window_without_destroying_the_reader_owner
    send(&mut state, Message::Play);
    let running = pid(&state);
    send(&mut state, Message::Game(GameMessage::HideForGame(running)));
    assert!(hidden(&state));
    assert!(!state.visible);
    assert!(play::exit_after_game(&state));
    // The game and its output reader stay.
    pump(&mut state, BOUND, |state| !state.log.is_empty());
    assert!(!idle(&state));
    send(&mut state, Message::Stop);
    pump(&mut state, BOUND, idle);

    // test_early_failure_and_stale_timer_do_not_hide_the_launcher: a game that already exited.
    let mut state = ready(dir.path(), &["--lines", "1", "--exit", "3"]);
    set(&mut state, "close_on_play", json!(true));
    send(&mut state, Message::Play);
    let gone = pid(&state);
    pump(&mut state, BOUND, idle);
    send(&mut state, Message::Game(GameMessage::HideForGame(gone)));
    assert!(state.visible);
    assert!(idle(&state));
    assert!(state.log.text().contains("The game exited (code 3)."));

    // A timer of an earlier start while another game runs.
    fake_game(
        &mut state,
        &["--lines", "1000", "--delay-ms", "50", "--exit", "0"],
    );
    send(&mut state, Message::Play);
    assert_ne!(pid(&state), gone);
    send(&mut state, Message::Game(GameMessage::HideForGame(gone)));
    assert!(state.visible);
    assert!(!hidden(&state));
    send(&mut state, Message::Stop);
    pump(&mut state, BOUND, idle);
    assert!(state.visible);
}

#[test]
fn a_hidden_launcher_exits_after_the_game() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(
        dir.path(),
        &["--lines", "3", "--delay-ms", "50", "--exit", "0"],
    );

    // test_visible_launcher_stays_open_after_game_exits
    send(&mut state, Message::Play);
    assert!(!play::exit_after_game(&state));
    pump(&mut state, BOUND, idle);
    assert!(state.visible);
    assert!(!play::exit_after_game(&state));

    // test_hidden_launcher_exits_after_game_output_is_finished
    send(&mut state, Message::Play);
    let running = pid(&state);
    send(&mut state, Message::Game(GameMessage::HideForGame(running)));
    assert!(play::exit_after_game(&state));
    pump(&mut state, BOUND, idle);
    // Every line arrived before the exit.
    let text = state.log.text();
    assert!(
        text.contains("line 3\n\nThe game exited (code 0)."),
        "{text}"
    );
    assert!(!play::exit_after_game(&state));
}

#[test]
fn run_log_is_written() {
    // test_log_path_follows_the_saves_folder_setting
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(dir.path(), &["--lines", "5", "--exit", "3"]);
    assert_eq!(
        state.paths.run_log_path(""),
        state.paths.data_dir.join("user").join("last_run.log")
    );
    let saves = dir.path().join("saves");
    assert_eq!(
        state.paths.run_log_path(&text_of(&saves)),
        saves.join("last_run.log")
    );

    // test_play_passes_the_run_log_to_the_output_reader and
    // test_play_log_is_kept_when_the_launcher_hides_on_play
    set(&mut state, "user_dir", json!(text_of(&saves)));
    let log: PathBuf = saves.join("last_run.log");
    for close_on_play in [false, true] {
        set(&mut state, "close_on_play", json!(close_on_play));
        send(&mut state, Message::Play);
        if close_on_play {
            let running = pid(&state);
            send(&mut state, Message::Game(GameMessage::HideForGame(running)));
        }
        pump(&mut state, BOUND, idle);
        let written = fs::read_to_string(&log).unwrap().replace("\r\n", "\n");
        assert_eq!(written, "line 1\nline 2\nline 3\nline 4\nline 5\n");
    }
    assert!(!dir.path().join("user").join("last_run.log").exists());
}

#[test]
fn play_offers_the_system_files_of_a_pkg_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("pkg").join("Image0");
    let sc0 = dir.path().join("pkg").join("Sc0");
    full_dump(&image);
    fs::remove_dir_all(image.join("sce_sys")).unwrap();
    fs::create_dir_all(&sc0).unwrap();
    fs::write(sc0.join("param.sfo"), game_sfo()).unwrap();
    fs::write(sc0.join("icon0.png"), b"png").unwrap();
    let mut state = state_in(dir.path());
    set(&mut state, "game_dir", json!(text_of(&image)));
    fake_game(&mut state, &["--lines", "1", "--exit", "0"]);

    send(&mut state, Message::Play);
    let asked = state.modal.clone().expect("the copy offer");
    let folder = text_of(&image);
    let ModalKind::YesNo { yes, no } = &asked.kind else {
        panic!("{asked:?}")
    };
    let Message::Game(GameMessage::AdoptAnswer(true, source, target)) = yes else {
        panic!("{yes:?}")
    };
    assert!(source.ends_with("Sc0"), "{source:?}");
    assert_eq!(target, &folder);
    assert!(
        matches!(no, Message::Game(GameMessage::AdoptAnswer(false, declined, from))
            if declined == source && from == &folder),
        "{no:?}"
    );
    assert_eq!(
        asked.text,
        format!(
            "The files in {folder} look like a PKG extraction: the system files (param.sfo, \
             icon0.png, pic1.png and the rest) are in\n\n{}\n\nCopy all of them into its sce_sys \
             folder now? Files already there are kept.",
            source.display()
        )
    );
    assert!(idle(&state));

    // No: PLAY goes on with its checks, as in Python, and the dump lacks its param.sfo.
    send(&mut state, no.clone());
    assert_eq!(
        error_text(&state),
        "The game dump is incomplete. Missing or empty files:\n\nsce_sys/param.sfo\n\nDump the \
         game again, or copy these files from a complete dump."
    );
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));
    // Nothing was copied.
    assert!(!image.join("sce_sys").exists());

    // PLAY again offers the copy again.
    send(&mut state, Message::Modal(ModalMessage::Dismiss));
    send(&mut state, Message::ShowPage(Page::Home));
    send(&mut state, Message::Play);
    assert_eq!(
        state.modal.clone().expect("the offer again").text,
        asked.text
    );

    // Yes: the copy runs in the background; PLAY waits for it.
    send(&mut state, yes.clone());
    assert!(state.modal.is_none());
    assert!(matches!(state.game, GameState::Copying));
    assert_eq!(home::chips(&state)[3], "Copying the system files…");
    assert!(click(&state, "PLAY").is_empty());
    send(&mut state, Message::Play);
    assert!(matches!(state.game, GameState::Copying));

    // A failed copy says why and opens the Game page.
    send(
        &mut state,
        Message::Game(GameMessage::AdoptDone(Err("disk full".to_string()))),
    );
    assert_eq!(error_text(&state), "Could not copy the files: disk full");
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));
    assert!(state.status.is_empty());

    // The copy started by Yes runs on its own thread; `param.sfo` lands last.
    let copied = image.join("sce_sys").join("param.sfo");
    let started = Instant::now();
    while fs::read(&copied).ok() != Some(game_sfo()) {
        assert!(
            started.elapsed() < BOUND,
            "the system files were not copied"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(image.join("sce_sys").join("icon0.png")).unwrap(),
        b"png"
    );

    // A finished copy presses PLAY again, which now starts the game.
    send(&mut state, Message::Modal(ModalMessage::Dismiss));
    send(&mut state, Message::Game(GameMessage::AdoptDone(Ok(()))));
    assert!(state.modal.is_none(), "{:?}", state.modal);
    assert!(
        matches!(state.game, GameState::Preparing { .. }),
        "{:?}",
        state.game
    );
    pump(&mut state, BOUND, idle);
    assert!(state.log.text().contains("line 1"));
}

#[test]
fn a_running_game_blocks_restore_reset_and_cache_clear() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(
        dir.path(),
        &["--lines", "1000", "--delay-ms", "50", "--exit", "0"],
    );
    assert!(!game::restore_blocked(&state));
    assert!(!advanced::clear_cache_blocked(&state));
    send(&mut state, Message::Play);
    assert!(game::restore_blocked(&state));
    assert!(advanced::clear_cache_blocked(&state));
    send(&mut state, Message::Trophies(TrophiesMessage::Reset));
    let shown = state.modal.take().unwrap();
    assert_eq!(shown.text, "Close the game before resetting the trophies.");
    send(
        &mut state,
        Message::Game(GameMessage::Output(vec![
            "Entering original x86-64 code\n".to_string()
        ])),
    );
    assert!(game::restore_blocked(&state));
    assert!(advanced::clear_cache_blocked(&state));
    send(&mut state, Message::Stop);
    pump(&mut state, BOUND, idle);
    assert!(!game::restore_blocked(&state));
    assert!(!advanced::clear_cache_blocked(&state));
}

#[test]
fn declining_the_game_offer_offers_the_update_files_next() {
    // Python `offer_system_files` goes on to the update folder after a No.
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("pkg").join("Image0");
    full_dump(&image);
    fs::remove_dir_all(image.join("sce_sys")).unwrap();
    fs::create_dir_all(dir.path().join("pkg").join("Sc0")).unwrap();
    fs::write(
        dir.path().join("pkg").join("Sc0").join("param.sfo"),
        game_sfo(),
    )
    .unwrap();
    let update = dir.path().join("upd").join("Image0");
    fs::create_dir_all(&update).unwrap();
    fs::write(update.join("eboot.bin"), b"elf").unwrap();
    let update_sc0 = dir.path().join("upd").join("Sc0");
    fs::create_dir_all(&update_sc0).unwrap();
    fs::write(update_sc0.join("param.sfo"), game_sfo()).unwrap();
    let mut state = state_in(dir.path());
    set(&mut state, "game_dir", json!(text_of(&image)));
    set(&mut state, "update_dir", json!(text_of(&update)));
    fake_game(&mut state, &["--lines", "1", "--exit", "0"]);

    send(&mut state, Message::Play);
    let game_offer = state.modal.clone().expect("the game folder's offer");
    let ModalKind::YesNo { no, .. } = &game_offer.kind else {
        panic!("{game_offer:?}")
    };
    assert!(game_offer
        .text
        .starts_with(&format!("The files in {} ", text_of(&image))));
    send(&mut state, no.clone());

    let update_offer = state.modal.clone().expect("the update folder's offer");
    let ModalKind::YesNo { yes, no } = &update_offer.kind else {
        panic!("{update_offer:?}")
    };
    assert!(
        update_offer
            .text
            .starts_with(&format!("The files in {} ", text_of(&update))),
        "{}",
        update_offer.text
    );
    let Message::Game(GameMessage::AdoptAnswer(true, source, target)) = yes else {
        panic!("{yes:?}")
    };
    assert_eq!(source, &update_sc0);
    assert_eq!(target, &text_of(&update));
    assert!(idle(&state));

    // No again: the checks name what the game folder lacks.
    send(&mut state, no.clone());
    assert!(
        error_text(&state).starts_with(
            "The game dump is incomplete. Missing or empty files:\n\nsce_sys/param.sfo\n\n"
        ),
        "{}",
        error_text(&state)
    );
    assert_eq!(state.page, Page::Game);
    assert!(idle(&state));
}

#[test]
fn a_restart_keeps_the_running_game() {
    // run.py's in-game restart: the probe says it restarts, then the new one enters the game again.
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(
        dir.path(),
        &["--lines", "1000", "--delay-ms", "50", "--exit", "0"],
    );
    send(&mut state, Message::Play);
    let running = pid(&state);
    send(&mut state, Message::Game(GameMessage::HideForGame(running)));
    let line = |text: &str| Message::Game(GameMessage::Output(vec![text.to_string()]));
    send(&mut state, line("Entering original x86-64 code\n"));
    send(&mut state, line("Runtime: restarting through run.py\n"));
    assert_eq!(state.status, "Restarting with the new settings…");
    send(&mut state, line("Entering original x86-64 code\n"));
    assert!(
        matches!(state.game, GameState::Running { hidden: true, .. }),
        "{:?}",
        state.game
    );
    assert_eq!(pid(&state), running);
    assert_eq!(state.status, "The game is running.");
    assert!(play::game_live(&state));
    assert!(play::exit_after_game(&state));
    assert!(game::restore_blocked(&state));
    // PLAY stays RUNNING, STOP stays.
    assert!(click(&state, "RUNNING").is_empty());
    assert!(click(&state, "STOP")
        .iter()
        .any(|message| matches!(message, Message::Stop)));
    // Both lines in one batch too.
    send(
        &mut state,
        Message::Game(GameMessage::Output(vec![
            "Runtime: restarting through run.py\n".to_string(),
            "Entering original x86-64 code\n".to_string(),
        ])),
    );
    assert_eq!(pid(&state), running);

    // The game's own output and its exit still arrive.
    send(&mut state, Message::Stop);
    pump(&mut state, BOUND, idle);
    let text = state.log.text();
    assert!(text.contains("line 1"), "{text}");
    assert!(text.contains("\n\nThe game exited (code "), "{text}");
}

#[test]
fn a_tick_drains_at_most_500_events() {
    // Python's drain_output takes at most 500 items per pass; the rest waits for the next one.
    let dir = tempfile::tempdir().unwrap();
    let mut state = ready(dir.path(), &["--lines", "1200", "--exit", "0"]);
    send(&mut state, Message::Play);
    let log = state.paths.run_log_path("");
    let started = Instant::now();
    while !fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("line 1200")
    {
        assert!(
            started.elapsed() < BOUND,
            "the fake game did not print in time"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let first = play::drain_events(&state);
    assert_eq!(first.len(), 1, "one Output, and no Exited yet");
    let GameMessage::Output(lines) = &first[0] else {
        panic!("not an Output")
    };
    assert_eq!(lines.len(), 500);
    assert_eq!(lines[0].trim_end(), "line 1");
    for message in first {
        send(&mut state, Message::Game(message));
    }
    assert!(!idle(&state));
    pump(&mut state, BOUND, idle);
    let shown: Vec<&str> = state.log.lines().collect();
    assert!(shown.contains(&"line 1200"));
    assert_eq!(
        shown.iter().filter(|l| l.starts_with("line ")).count(),
        1200
    );
}
