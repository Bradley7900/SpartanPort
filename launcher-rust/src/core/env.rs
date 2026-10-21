//! The environment the game is started with (`SPARTAN_*`) and the trophy texts it reads.

use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use serde_json::Value;

use crate::core::controls::{control_map, keyboard_map, Kind};
use crate::core::game::{game_profile_of, meta_folder};
use crate::core::i18n::{tr, trophy_text};
use crate::core::paths::Paths;
use crate::core::settings::Settings;

const TROPHY_COUNT: usize = 35;

/// Python truthiness of a saved value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `settings.get(key, default)` read as a flag.
fn flag(settings: &Settings, key: &str, default: bool) -> bool {
    settings.get(key).map_or(default, truthy)
}

fn one_or_zero(on: bool) -> String {
    if on { "1" } else { "0" }.to_string()
}

/// `user/trophy_texts.txt`: the trophy names, descriptions and labels the game shows, in the
/// launcher's language. A failure is harmless.
pub fn write_trophy_texts(folder: &Path, lang: &str) {
    let labels = [
        ("heading", "Trophies"),
        ("notice", "TROPHY UNLOCKED"),
        ("unlocked", "UNLOCKED"),
        ("locked", "LOCKED"),
        ("all", "All trophies"),
        ("only_unlocked", "Unlocked"),
        ("only_locked", "Locked"),
        ("counter", "{} of {} unlocked"),
        (
            "hint",
            "Up/Down: scroll   Left/Right: filter   F8 / R3+R2 / Esc: close",
        ),
    ];
    let mut lines = vec![format!(
        "# Written by the launcher on every start (language: {lang}); do not edit."
    )];
    for (key, en) in labels {
        let text = tr(lang, en);
        let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        lines.push(format!("@{key}={collapsed}"));
    }
    for trophy in 0..TROPHY_COUNT {
        let (name, description) = trophy_text(lang, trophy);
        lines.push(format!("{trophy}={name} | {description}"));
    }
    if fs::create_dir_all(folder).is_ok() {
        let _ = fs::write(folder.join("trophy_texts.txt"), lines.join("\n") + "\n");
    }
}

/// `base` (the launcher's own environment) plus everything the game reads from it.
pub fn game_environment(
    settings: &Settings,
    paths: &Paths,
    lang: &str,
    base: &IndexMap<String, String>,
    windows: bool,
) -> IndexMap<String, String> {
    let mut env = base.clone();
    write_trophy_texts(&paths.user_dir(&settings.get_str("user_dir")), lang);
    env.insert("SPARTAN_GAME_DIR".into(), settings.get_str("game_dir"));
    env.shift_remove("SPARTAN_PATCH_DIR");
    // Temporal upscaling is parked: a switch saved on earlier has no control left to turn it off.
    env.insert("SPARTAN_EXPERIMENTAL_UPSCALER".into(), "0".into());
    let update_dir = settings.get_str("update_dir");
    if !update_dir.is_empty() {
        env.insert("SPARTAN_PATCH_DIR".into(), update_dir.clone());
    }
    if !windows {
        let backend = settings
            .get("video_backend")
            .and_then(Value::as_str)
            .unwrap_or("auto");
        if backend == "wayland" || backend == "x11" {
            env.insert("SDL_VIDEO_DRIVER".into(), backend.into());
            env.insert("SDL_VIDEODRIVER".into(), backend.into());
        }
        env.insert(
            "SPARTAN_GAMEMODE".into(),
            one_or_zero(flag(settings, "gamemode", false)),
        );
        env.insert(
            "SPARTAN_MANGOHUD".into(),
            one_or_zero(flag(settings, "mangohud", false)),
        );
    }
    // The launcher's patch selection is the whole list.
    let game_dir = settings.get_str("game_dir");
    let (title_id, profile) = game_profile_of(&meta_folder(&game_dir, &update_dir));
    let chosen = title_id
        .as_deref()
        .and_then(|id| settings.get("game_patches")?.get(id)?.as_array());
    if let (Some(_), Some(chosen)) = (profile, chosen) {
        let names: Vec<&str> = chosen.iter().filter_map(Value::as_str).collect();
        env.insert("SPARTAN_PATCHES".into(), names.join(";"));
        env.insert("SPARTAN_PATCHES_ONLY".into(), "1".into());
    }
    let user_dir = settings.get_str("user_dir");
    if !user_dir.is_empty() {
        env.insert("SPARTAN_USER_DIR".into(), user_dir);
    }
    if !windows {
        env.insert(
            "SPARTAN_FSR4_DIR".into(),
            paths.fsr4_folder().to_string_lossy().into_owned(),
        );
    }
    let path_text = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
    env.insert(
        "SPARTAN_MODS_DIR".into(),
        path_text(paths.mods_dir(&settings.get_str("mods_dir"))),
    );
    env.insert("SPARTAN_MODS_CONFIG".into(), path_text(paths.mods_json()));
    env.insert(
        "SPARTAN_MODS_ENABLED".into(),
        one_or_zero(flag(settings, "mods_enabled", false)),
    );
    env.insert(
        "SPARTAN_PATCHES_DIR".into(),
        path_text(paths.patches_dir(&settings.get_str("patches_dir"))),
    );
    env.insert(
        "SPARTAN_PATCHES_CONFIG".into(),
        path_text(paths.patches_json()),
    );
    env.insert("SPARTAN_LANGUAGE".into(), settings.get_str("language"));
    // Menu language is independent from the game's text and voice language.
    let ui_language = settings.get_str("ui_language");
    env.insert(
        "SPARTAN_UI_LANGUAGE".into(),
        if ui_language.is_empty() {
            lang.to_string()
        } else {
            ui_language
        },
    );
    env.insert(
        "SPARTAN_DISCORD".into(),
        one_or_zero(flag(settings, "discord_rpc", false)),
    );
    if flag(settings, "hdr", false) {
        env.insert("SPARTAN_HDR".into(), "1".into());
    }
    for (key, name) in [
        ("draw_pipe", "SPARTAN_DRAW_PIPE"),
        ("readbacks", "SPARTAN_READBACKS"),
        ("no_msaa", "SPARTAN_NO_MSAA"),
    ] {
        if flag(settings, key, false) {
            env.insert(name.into(), settings.get_str(key));
        }
    }
    // The saved UI choice wins over inherited diagnostic overrides, including when reverting.
    let stream_memory = if settings.get("stream_memory").is_some() {
        settings.get_str("stream_memory")
    } else {
        "cached".to_string()
    };
    let stream_memory = if ["cached", "host", "device"].contains(&stream_memory.as_str()) {
        stream_memory
    } else {
        "cached".to_string()
    };
    env.insert("SPARTAN_STREAM_MEMORY".into(), stream_memory);
    // Only ASCII digits: the game cannot read the other Unicode digits.
    let digits = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    let gpu_id = settings.get_str("gpu_id");
    env.shift_remove("SPARTAN_GPU_ID");
    if digits(&gpu_id) {
        env.insert("SPARTAN_GPU_ID".into(), gpu_id);
    }
    // The monitor index the probe enumerated; an empty choice is the primary display.
    let monitor = settings.get_str("monitor");
    env.shift_remove("SPARTAN_MONITOR");
    if digits(&monitor) {
        env.insert("SPARTAN_MONITOR".into(), monitor);
    }
    env.insert(
        "SPARTAN_BACKGROUND_WARMUP".into(),
        one_or_zero(flag(settings, "background_warmup", true)),
    );
    env.insert(
        "SPARTAN_PARALLEL_WARMUP".into(),
        one_or_zero(flag(settings, "parallel_warmup", true)),
    );
    env.insert(
        "SPARTAN_PIPELINE_BINARIES".into(),
        one_or_zero(flag(settings, "pipeline_binary_cache", true)),
    );
    env.insert(
        "SPARTAN_PERF_DIAG".into(),
        one_or_zero(flag(settings, "perf_diag", false)),
    );
    env.insert(
        "SPARTAN_PARTIAL_HEIGHT_VIEWS".into(),
        one_or_zero(flag(settings, "partial_height_views", true)),
    );
    for (key, name) in [
        ("frame_stats", "SPARTAN_FRAME_STATS"),
        ("gpu_profile", "SPARTAN_GPU_PROFILE"),
        ("vk_validation", "SPARTAN_VK_VALIDATION"),
        ("frame_capture_key", "SPARTAN_FRAME_CAPTURE_KEY"),
    ] {
        if flag(settings, key, false) {
            env.insert(name.into(), "1".into());
        }
    }
    let no_controls = Value::Object(serde_json::Map::new());
    let controls = settings.get("controls").unwrap_or(&no_controls);
    let value = keyboard_map(controls, flag(settings, "rage_one_input", true));
    if !value.is_empty() {
        env.insert("SPARTAN_KEY_MAP".into(), value);
    }
    let value = control_map(controls, Kind::Pad);
    if !value.is_empty() {
        env.insert("SPARTAN_PAD_MAP".into(), value);
    }
    env.shift_remove("SPARTAN_STICK_PROMPTS");
    let stick_prompts = settings.get_str("stick_prompts");
    if stick_prompts == "keyboard" || stick_prompts == "mouse" {
        env.insert("SPARTAN_STICK_PROMPTS".into(), stick_prompts);
    }
    let glyph_style = settings.get_str("glyph_style");
    env.shift_remove("SPARTAN_GLYPH_STYLE");
    if ["auto", "xbox", "nintendo", "keyboard"].contains(&glyph_style.as_str()) {
        env.insert("SPARTAN_GLYPH_STYLE".into(), glyph_style);
    }
    env.insert("PYTHONUNBUFFERED".into(), "1".into());
    env.insert("PYTHONIOENCODING".into(), "utf-8".into());
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::settings::app_defaults;
    use serde_json::json;

    fn setup(edits: Value) -> (Settings, Paths, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_string_lossy().into_owned();
        let env = move |key: &str| (key == "SPARTAN_DATA_DIR").then(|| data.clone());
        let paths = Paths::from_env(dir.path(), &env);
        let mut settings = Settings(app_defaults(dir.path(), true, &|_| None));
        if let Value::Object(map) = edits {
            for (key, value) in map {
                settings.set(&key, value);
            }
        }
        (settings, paths, dir)
    }

    fn base(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn inherited_gpu_id_and_monitor_are_dropped_when_not_digits() {
        let (settings, paths, _dir) = setup(json!({"gpu_id": "abc", "monitor": "-1"}));
        let inherited = base(&[("SPARTAN_GPU_ID", "7"), ("SPARTAN_MONITOR", "3")]);
        let env = game_environment(&settings, &paths, "en", &inherited, true);
        assert!(!env.contains_key("SPARTAN_GPU_ID"));
        assert!(!env.contains_key("SPARTAN_MONITOR"));

        let (settings, paths, _dir) = setup(json!({"gpu_id": "2", "monitor": "1"}));
        let env = game_environment(&settings, &paths, "en", &inherited, true);
        assert_eq!(env["SPARTAN_GPU_ID"], "2");
        assert_eq!(env["SPARTAN_MONITOR"], "1");
    }

    #[test]
    fn stream_memory_falls_back_to_cached() {
        let (settings, paths, _dir) = setup(json!({"stream_memory": "weird"}));
        let inherited = base(&[("SPARTAN_STREAM_MEMORY", "device")]);
        let env = game_environment(&settings, &paths, "en", &inherited, true);
        assert_eq!(env["SPARTAN_STREAM_MEMORY"], "cached");
        let (settings, paths, _dir) = setup(json!({"stream_memory": "host"}));
        let env = game_environment(&settings, &paths, "en", &inherited, true);
        assert_eq!(env["SPARTAN_STREAM_MEMORY"], "host");
    }

    #[test]
    fn trophy_texts_file_has_35_rows_and_the_labels() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("user");
        write_trophy_texts(&folder, "pt");
        let text = fs::read_to_string(folder.join("trophy_texts.txt")).unwrap();
        assert!(text.ends_with('\n'));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 45);
        assert!(lines[0].starts_with("# Written by the launcher on every start (language: pt)"));
        assert_eq!(lines[1], format!("@heading={}", tr("pt", "Trophies")));
        let (name, _) = trophy_text("pt", 0);
        assert!(lines[10].starts_with(&format!("0={name} | ")));
        assert!(lines[44].starts_with("34="));
    }
}
