//! The launcher's own settings (`settings.json`): defaults, load, save and the portable migration.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::core::fs::{atomic_write, load_json};
use crate::core::paths::Paths;

/// The saved settings, in file order; keys this version does not know are kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings(pub Map<String, Value>);

impl Settings {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Strings as-is, numbers as their text, anything else empty.
    pub fn get_str(&self, key: &str) -> String {
        match self.0.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    }

    /// `true`, the number 1 or the string "1".
    pub fn get_bool(&self, key: &str) -> bool {
        match self.0.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_i64() == Some(1) || n.as_f64() == Some(1.0),
            Some(Value::String(s)) => s == "1",
            _ => false,
        }
    }

    /// Replacing a key keeps its position; a new key goes last.
    pub fn set(&mut self, key: &str, value: Value) {
        self.0.insert(key.to_string(), value);
    }
}

/// Every setting with its default, in the order the Python launcher writes them.
pub fn app_defaults(
    port_dir: &Path,
    windows: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Map<String, Value> {
    let game_dir = env("SPARTAN_GAME_DIR").unwrap_or_else(|| {
        if windows {
            port_dir
                .parent()
                .unwrap_or(port_dir)
                .join("CUSA01623")
                .to_string_lossy()
                .into_owned()
        } else {
            String::new()
        }
    });
    let defaults = json!({
        "ui_language": "en",
        "ui_scale": "auto",
        "game_dir": game_dir,
        "update_dir": "",
        "experimental_upscaler": false,
        "video_backend": "auto",
        "gamemode": false,
        "mangohud": false,
        "desktop_integration": true,
        "user_dir": "",
        "mods_dir": "",
        "mods_enabled": true,
        "patches_dir": "",
        "language": "1",
        "hdr": false,
        "draw_pipe": "",
        "readbacks": "",
        "no_msaa": "",
        "stream_memory": "cached",
        "gpu_id": "",
        "monitor": "",
        "partial_height_views": true,
        "background_warmup": true,
        "parallel_warmup": true,
        "pipeline_binary_cache": true,
        "perf_diag": false,
        "frame_capture_key": false,
        "frame_stats": false,
        "gpu_profile": false,
        "vk_validation": false,
        "close_on_play": false,
        "discord_rpc": false,
        "check_updates": false,
        "game_patches": {},
        "controls": {},
        "pad_style": "playstation",
        "rage_one_input": true,
        "stick_prompts": "keyboard",
        "controls_layout": 2,
        "glyph_style": "off",
    });
    match defaults {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Upgrades the saved control bindings to the current layout.
///
/// Placeholder: Task 11 replaces this with the port of the Python `migrate_controls`. The Python
/// returns its input unchanged when it is empty or `controls_layout` is already 2.
pub fn migrate_controls(saved: Map<String, Value>) -> Map<String, Value> {
    saved
}

/// Copies the previous launcher's `settings.json` and `game_icon.ico` into the data folder, once.
/// Never overwrites; every error is ignored.
pub fn migrate_portable_settings(target: &Path, legacy: &Path) {
    if target == legacy {
        return;
    }
    copy_if_missing(legacy, target);
    if let (Some(target_dir), Some(legacy_dir)) = (target.parent(), legacy.parent()) {
        copy_if_missing(
            &legacy_dir.join("game_icon.ico"),
            &target_dir.join("game_icon.ico"),
        );
    }
}

fn copy_if_missing(from: &Path, to: &Path) {
    if to.is_file() || !from.is_file() {
        return;
    }
    if let Some(parent) = to.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::copy(from, to);
}

/// Defaults overlaid with the saved file. A file that is missing, invalid or not a JSON object
/// gives the defaults and is left untouched until the next save.
pub fn load_app_settings(paths: &Paths) -> Settings {
    let config = paths.config_file();
    migrate_portable_settings(&config, &paths.legacy_config_file());
    let saved = match load_json::<Value>(&config, Value::Null) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let mut merged = app_defaults(&paths.port_dir, cfg!(windows), &|k| std::env::var(k).ok());
    for (key, value) in migrate_controls(saved) {
        merged.insert(key, value);
    }
    Settings(merged)
}

/// Writes the settings as 2-space JSON without a trailing newline.
pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let text = serde_json::to_string_pretty(&settings.0).map_err(io::Error::other)?;
    atomic_write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::Paths;
    use serde_json::json;
    use std::fs;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_are_in_python_order() {
        let defaults = app_defaults(Path::new("/port"), true, &no_env);
        assert_eq!(defaults.len(), 40);
        assert_eq!(defaults.keys().next().unwrap(), "ui_language");
        assert_eq!(defaults.keys().next_back().unwrap(), "glyph_style");
        assert_eq!(defaults["game_dir"], json!("/CUSA01623"));
        let linux = app_defaults(Path::new("/port"), false, &no_env);
        assert_eq!(linux["game_dir"], json!(""));
        let env = |k: &str| (k == "SPARTAN_GAME_DIR").then(|| "/games".to_string());
        assert_eq!(
            app_defaults(Path::new("/port"), false, &env)["game_dir"],
            json!("/games")
        );
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let saved = json!({"zzz_future": {"a": [1, 2]}, "ui_language": "pt"});
        fs::write(&path, saved.to_string()).unwrap();
        let paths = Paths::from_env(dir.path(), &|k| {
            (k == "SPARTAN_DATA_DIR").then(|| dir.path().to_string_lossy().into_owned())
        });
        let mut settings = load_app_settings(&paths);
        assert_eq!(settings.get_str("ui_language"), "pt");
        settings.set("hdr", json!(true));
        save_settings(&path, &settings).unwrap();
        let back: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(back["zzz_future"], json!({"a": [1, 2]}));
        assert_eq!(back["hdr"], json!(true));
        assert_eq!(
            back.as_object().unwrap().keys().next_back().unwrap(),
            "zzz_future"
        );
    }

    #[test]
    fn loading_a_json_array_or_string_falls_back_to_defaults() {
        for text in ["[]", "\"x\""] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(&path, text).unwrap();
            let data = dir.path().to_string_lossy().into_owned();
            let paths = Paths::from_env(dir.path(), &|k| match k {
                "SPARTAN_DATA_DIR" => Some(data.clone()),
                "APPDATA" | "XDG_CONFIG_HOME" => Some(format!("{data}/nowhere")),
                _ => None,
            });
            let settings = load_app_settings(&paths);
            let defaults = app_defaults(&paths.port_dir, cfg!(windows), &|k| std::env::var(k).ok());
            assert_eq!(settings.0, defaults);
            assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
        }
    }

    #[test]
    fn portable_migration_copies_once_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_dir = dir.path().join("legacy");
        let target_dir = dir.path().join("data");
        fs::create_dir_all(&legacy_dir).unwrap();
        let legacy = legacy_dir.join("settings.json");
        let target = target_dir.join("settings.json");
        fs::write(&legacy, "old").unwrap();
        fs::write(legacy_dir.join("game_icon.ico"), "ico").unwrap();

        migrate_portable_settings(&target, &legacy);
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(target_dir.join("game_icon.ico")).unwrap(),
            "ico"
        );

        fs::write(&legacy, "newer").unwrap();
        fs::write(target_dir.join("game_icon.ico"), "mine").unwrap();
        migrate_portable_settings(&target, &legacy);
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(target_dir.join("game_icon.ico")).unwrap(),
            "mine"
        );

        migrate_portable_settings(&legacy, &legacy);
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "newer");
    }
}
