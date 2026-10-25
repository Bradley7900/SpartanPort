#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The views read from the disk follow the settings they are read from (`ui::follow`).

mod common;

use std::path::{Path, PathBuf};

use common::{
    dump_with_patches, load_mods, load_trophies, make_mod, state, state_with_trophies, trp_package,
    write_mod_patch,
};
use iced::futures::executor::block_on;
use iced::futures::StreamExt;
use iced::Task;
use iced_test::runtime::task::into_stream;
use iced_test::runtime::Action;
use serde_json::Value;
use spartanport_launcher::core::trophies::TROPHY_COUNT;
use spartanport_launcher::ui::app::{self, Message, State};
use spartanport_launcher::ui::follow::{inputs, View};
use spartanport_launcher::ui::pages::mods::{self, ModsMessage};
use spartanport_launcher::ui::pages::trophies::{self, TrophiesMessage};

/// Runs what `task` reads in the background and hands each message back to `update`, with what
/// those start in turn, as the window's runtime does.
fn settle(state: &mut State, task: Task<Message>) {
    let Some(stream) = into_stream(task) else {
        return;
    };
    for action in block_on(stream.collect::<Vec<_>>()) {
        if let Action::Output(message) = action {
            let next = app::update(state, message);
            settle(state, next);
        }
    }
}

/// `Message::Set` of a text, with what it reads in the background.
fn set(state: &mut State, key: &str, value: &str) {
    let task = app::update(state, Message::Set(key.into(), Value::String(value.into())));
    settle(state, task);
}

/// `Message::Browsed`, as the dialog or a keystroke in the folder field sends it.
fn browse(state: &mut State, key: &str, path: &Path) {
    let task = app::update(
        state,
        Message::Browsed(key.into(), Some(path.to_path_buf())),
    );
    settle(state, task);
}

/// Every view, in the table's order.
const VIEWS: [View; 8] = [
    View::Installation,
    View::Home,
    View::Patches,
    View::Backups,
    View::ModResolutions,
    View::Aspect,
    View::Mods,
    View::Trophies,
];

fn text(path: &Path) -> String {
    path.display().to_string()
}

/// What `view` shows, as text to compare.
fn look(state: &State, view: View) -> String {
    match view {
        View::Installation => format!(
            "{:?} {:?}",
            state.installation.title_id(),
            state.installation.has_game()
        ),
        View::Home => format!("{:?}", state.home),
        View::Patches => format!(
            "{:?} {:?}",
            state.patches.title_id,
            state
                .patches
                .selectable
                .iter()
                .map(|meta| &meta.name)
                .collect::<Vec<_>>()
        ),
        View::Backups => format!("{:?}", state.backups),
        View::ModResolutions => format!("{:?}", state.mod_resolutions),
        View::Aspect => state.aspect.clone(),
        View::Mods => format!("{:?}", state.mods.order),
        View::Trophies => format!(
            "{} {:?}",
            state.trophies.icons.len(),
            state.trophies.unlocked
        ),
    }
}

/// For one view: what changes on the disk it reads, a setting it does not read and one it reads,
/// each with a new value that does not change the view by itself.
struct Case {
    disk: fn(&State, &Path),
    outside: (&'static str, fn(&State, &Path) -> String),
    inside: (&'static str, fn(&State, &Path) -> String),
}

fn case(view: View) -> Case {
    // An empty folder or a missing one: nothing the views read is there.
    let saves = |_: &State, dir: &Path| text(&dir.join("saves"));
    let mods_folder = |_: &State, dir: &Path| text(&dir.join("other-mods"));
    let update = |_: &State, dir: &Path| text(&dir.join("update"));
    match view {
        View::Installation => Case {
            disk: |_, dir| {
                dump_with_patches(dir);
            },
            outside: ("user_dir", saves),
            inside: ("update_dir", update),
        },
        View::Home => Case {
            disk: |state, _| {
                let savedata = state.paths.user_dir("").join("savedata");
                std::fs::create_dir_all(&savedata).unwrap();
                std::fs::write(savedata.join("slot0"), b"x").unwrap();
            },
            outside: ("mods_dir", mods_folder),
            inside: ("update_dir", update),
        },
        View::Patches => Case {
            disk: |_, dir| {
                dump_with_patches(dir);
            },
            outside: ("user_dir", saves),
            inside: ("update_dir", update),
        },
        View::Backups => Case {
            disk: |state, _| {
                let backups = state.paths.user_dir("").join("savedata_backups");
                std::fs::create_dir_all(backups.join("20261009-180000")).unwrap();
            },
            // Spec §4/06 (changed): the game folder no longer lists the backups again.
            outside: ("game_dir", |_, dir| text(&dir.join("other-game"))),
            // The same saves folder, spelled out.
            inside: ("user_dir", |state, _| text(&state.paths.user_dir(""))),
        },
        View::ModResolutions => Case {
            disk: |state, _| {
                let patches = state.paths.mods_dir("").join("Ultrawide").join("patches");
                std::fs::create_dir_all(&patches).unwrap();
                write_mod_patch(&patches, "3840x1080");
            },
            // Not a setting of the Mods page either: a read of its lists also reads the sizes.
            outside: ("user_dir", saves),
            // The same mods folder, spelled out.
            inside: ("mods_dir", |state, _| text(&state.paths.mods_dir(""))),
        },
        View::Aspect => Case {
            // It reads only the render resolution, no file.
            disk: |_, _| {},
            outside: ("user_dir", saves),
            inside: ("render_resolution", |_, _| "3440x1440".into()),
        },
        View::Mods => Case {
            disk: |state, _| make_mod(state, "Ultrawide", "a.bin"),
            outside: ("user_dir", saves),
            inside: ("patches_dir", |_, dir| text(&dir.join("other-patches"))),
        },
        View::Trophies => Case {
            disk: |_, dir| {
                let trophy = dir.join("dump").join("sce_sys").join("trophy");
                std::fs::create_dir_all(&trophy).unwrap();
                std::fs::write(trophy.join("trophy00.trp"), trp_package(TROPHY_COUNT)).unwrap();
            },
            outside: ("mods_dir", mods_folder),
            inside: ("update_dir", update),
        },
    }
}

#[test]
fn each_view_follows_exactly_its_keys() {
    let mut wrong = Vec::new();
    for view in VIEWS {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state(dir.path());
        // A game folder, still empty, as every view reads it.
        let game = dir.path().join("dump");
        std::fs::create_dir_all(&game).unwrap();
        browse(&mut state, "game_dir", &game);
        let Case {
            disk,
            outside,
            inside,
        } = case(view);
        let before = look(&state, view);
        disk(&state, dir.path());
        let value = (outside.1)(&state, dir.path());
        set(&mut state, outside.0, &value);
        if look(&state, view) != before {
            wrong.push(format!("{view:?} followed {}", outside.0));
        }
        let value = (inside.1)(&state, dir.path());
        set(&mut state, inside.0, &value);
        if look(&state, view) == before {
            wrong.push(format!("{view:?} did not follow {}", inside.0));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Everything the views show, as text to compare.
fn views(state: &State) -> String {
    let shown: Vec<String> = VIEWS.iter().map(|view| look(state, *view)).collect();
    format!(
        "{shown:?} {:?} {:?} {:?}",
        state.backup_choice, state.mods.inputs, state.trophies.inputs
    )
}

#[test]
fn typing_and_browsing_a_folder_give_the_same_views() {
    // A folder chosen in the dialog or typed into its field is `Browsed`; `Set` of the same path
    // is the same change.
    let dir = tempfile::tempdir().unwrap();
    let mut typed = state(dir.path());
    let mut browsed = state(dir.path());
    let game = dump_with_patches(dir.path());
    std::fs::create_dir_all(game.join("sce_sys/trophy")).unwrap();
    std::fs::write(
        game.join("sce_sys/trophy/trophy00.trp"),
        trp_package(TROPHY_COUNT),
    )
    .unwrap();
    let saves = dir.path().join("saves");
    std::fs::create_dir_all(saves.join("savedata")).unwrap();
    std::fs::write(saves.join("savedata").join("slot0"), b"x").unwrap();
    std::fs::create_dir_all(saves.join("savedata_backups").join("20261009-180000")).unwrap();
    let mods_dir = dir.path().join("my-mods");
    let patches = mods_dir.join("Ultrawide").join("patches");
    std::fs::create_dir_all(&patches).unwrap();
    write_mod_patch(&patches, "3840x1080");
    let folders = [
        ("game_dir", game.clone()),
        ("update_dir", dir.path().join("update")),
        ("user_dir", saves),
        ("mods_dir", mods_dir),
        ("patches_dir", dir.path().join("my-patches")),
    ];
    for (key, folder) in folders {
        let before = views(&typed);
        set(&mut typed, key, &text(&folder));
        browse(&mut browsed, key, &folder);
        assert_eq!(views(&typed), views(&browsed), "{key}");
        assert_ne!(views(&typed), before, "nothing followed {key}");
    }
    assert_eq!(typed.installation.title_id(), Some("CUSA01623"));
    assert_eq!(typed.mods.order, ["Ultrawide"]);
    assert_eq!(typed.trophies.icons.len(), TROPHY_COUNT);
}

#[test]
fn typing_a_game_folder_letter_by_letter_keeps_the_last_read() {
    // Review Focus 3: each keystroke sends `Browsed` and starts a read of the Mods and Trophies
    // lists. They finish in any order; only the read for the folder as typed last is kept.
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    let (mut stale_mods, mut stale_trophies) = (Vec::new(), Vec::new());
    for typed in ["C:/g", "C:/ga", "C:/game"] {
        let _ = app::update(
            &mut state,
            Message::Browsed("game_dir".into(), Some(PathBuf::from(typed))),
        );
        stale_mods.push(mods::load(&state.paths, &state.store, &state.installation));
        stale_trophies.push(trophies::load(
            &state.paths,
            &state.store,
            &state.installation,
        ));
    }
    // The read for the whole folder finishes first, the older ones after it, newest first.
    stale_mods.pop();
    stale_trophies.pop();
    load_mods(&mut state);
    load_trophies(&mut state);
    for (mods, trophies) in stale_mods.into_iter().zip(stale_trophies).rev() {
        let _ = app::update(
            &mut state,
            Message::Mods(ModsMessage::Loaded(Box::new(mods))),
        );
        let _ = app::update(
            &mut state,
            Message::Trophies(TrophiesMessage::Loaded(Box::new(trophies))),
        );
    }
    assert_eq!(state.store.text("game_dir"), "C:/game");
    assert_eq!(state.mods.inputs, inputs(View::Mods, &state.store));
    assert_eq!(state.mods.inputs[1], "C:/game");
    assert_eq!(state.trophies.inputs, inputs(View::Trophies, &state.store));
    assert_eq!(state.trophies.inputs[0], "C:/game");
}

#[test]
fn a_read_for_a_folder_typed_over_is_dropped() {
    // N2: each keystroke in a folder field starts a read of the Trophies and Mods lists, and the
    // reads finish in any order: one made for a path since typed over must not replace the list.
    use spartanport_launcher::core::installation::Installation;
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_with_trophies(dir.path());
    make_mod(&state, "Ultrawide", "a.bin");
    let game = state.store.text("game_dir");
    let mods_dir = state.paths.mods_dir("").display().to_string();
    // The reads started while the fields held one character less.
    state
        .store
        .set("game_dir", game[..game.len() - 1].to_string().into());
    state.store.set(
        "mods_dir",
        mods_dir[..mods_dir.len() - 1].to_string().into(),
    );
    let stale = Installation::of(&state.store);
    let stale_trophies = trophies::load(&state.paths, &state.store, &stale);
    let stale_mods = mods::load(&state.paths, &state.store, &stale);
    assert!(stale_trophies.icons.is_empty() && stale_mods.order.is_empty());
    state.store.set("game_dir", game.into());
    state.store.set("mods_dir", mods_dir.into());
    assert_ne!(stale_trophies.inputs, inputs(View::Trophies, &state.store));
    assert_ne!(stale_mods.inputs, inputs(View::Mods, &state.store));
    // The reads for the whole paths finish first, the older ones after them.
    load_trophies(&mut state);
    load_mods(&mut state);
    let _ = app::update(
        &mut state,
        Message::Trophies(TrophiesMessage::Loaded(Box::new(stale_trophies))),
    );
    let _ = app::update(
        &mut state,
        Message::Mods(ModsMessage::Loaded(Box::new(stale_mods))),
    );
    assert_eq!(state.trophies.icons.len(), 35);
    assert_eq!(state.mods.order, ["Ultrawide"]);
}
