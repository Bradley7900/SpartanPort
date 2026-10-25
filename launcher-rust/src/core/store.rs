//! The two settings files in one value: `settings.json` and the game's `spartanport.ini`.

use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::Value;

use crate::core::catalog::{setting, File, Kind, SETTINGS};
use crate::core::ini::{read_ini, Ini};
use crate::core::paths::Paths;
use crate::core::settings::{read_app_settings, Settings, SettingsFile};

pub struct Store {
    app: Settings,
    ini: Ini,
}

/// A value as ini text: a bool is `"1"`/`"0"`, a string stays, the rest is written out.
fn ini_text(value: Value) -> String {
    match value {
        Value::Bool(on) => if on { "1" } else { "0" }.to_string(),
        Value::String(text) => text,
        other => other.to_string(),
    }
}

/// The keys whose value differs between two maps, in `new` order and then those only in `old`.
fn changed<'a, V: PartialEq + 'a>(
    old: impl Fn(&str) -> Option<&'a V>,
    old_keys: impl Iterator<Item = &'a String>,
    new: impl Iterator<Item = (&'a String, &'a V)>,
    new_has: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut keys = Vec::new();
    for (key, value) in new {
        if old(key) != Some(value) {
            keys.push(key.clone());
        }
    }
    for key in old_keys {
        if !new_has(key) {
            keys.push(key.clone());
        }
    }
    keys
}

impl Store {
    /// Reads both files, plus the ones that are there but could not be read (the files stay as
    /// they are until a save).
    pub fn load(paths: &Paths) -> (Store, Vec<PathBuf>) {
        let (app, settings_file) = read_app_settings(paths);
        let (ini, ini_unreadable) = read_ini(&paths.ini_path());
        let unreadable = [
            (
                settings_file == SettingsFile::Unreadable,
                paths.config_file(),
            ),
            (ini_unreadable, paths.ini_path()),
        ]
        .into_iter()
        .filter_map(|(failed, file)| failed.then_some(file))
        .collect();
        (Store { app, ini }, unreadable)
    }

    pub fn flag(&self, key: &str) -> bool {
        match setting(key) {
            Some(row) if row.file == File::Ini => self
                .ini
                .values
                .get(key)
                .map_or_else(|| row.default.is_on(), |value| value == "1"),
            _ => self.app.flag(key),
        }
    }

    pub fn text(&self, key: &str) -> String {
        match setting(key) {
            Some(row) if row.file == File::Ini => self
                .ini
                .values
                .get(key)
                .cloned()
                .unwrap_or_else(|| row.default.as_text()),
            _ => self.app.text(key),
        }
    }

    /// The raw `settings.json` value; an ini key has none.
    pub fn json(&self, key: &str) -> Option<&Value> {
        self.app.get(key)
    }

    /// Stores `value` where the key lives (an unknown key goes to `settings.json`); whether the
    /// stored value changed.
    pub fn set(&mut self, key: &str, value: Value) -> bool {
        match setting(key) {
            Some(row) if row.file == File::Ini => {
                let text = ini_text(value);
                let same = self.ini.values.get(key) == Some(&text);
                self.ini.values.insert(key.to_string(), text);
                !same
            }
            _ => {
                let same = self.app.get(key) == Some(&value);
                self.app.set(key, value);
                !same
            }
        }
    }

    /// Swaps in both files' contents; the keys whose value changed.
    pub fn replace(&mut self, app: Settings, ini: Ini) -> Vec<String> {
        let mut keys = changed(
            |key| self.app.0.get(key),
            self.app.0.keys(),
            app.0.iter(),
            |key| app.0.contains_key(key),
        );
        keys.extend(changed(
            |key| self.ini.values.get(key),
            self.ini.values.keys(),
            ini.values.iter(),
            |key| ini.values.contains_key(key),
        ));
        self.app = app;
        self.ini = ini;
        keys
    }

    pub fn app(&self) -> &Settings {
        &self.app
    }

    pub fn ini(&self) -> &Ini {
        &self.ini
    }

    /// What a save writes to the ini: the keys the window has a control for, a switch as
    /// `"1"`/`"0"` (anything but "1" reads as off).
    pub fn ini_to_write(&self) -> IndexMap<String, String> {
        SETTINGS
            .iter()
            .filter(|row| row.file == File::Ini && row.written)
            .map(|row| {
                let value = if matches!(row.kind, Kind::Flag) {
                    if self.flag(row.key) { "1" } else { "0" }.to_string()
                } else {
                    self.text(row.key)
                };
                (row.key.to_string(), value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fresh() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let (store, unreadable) = Store::load(&Paths::from_env(dir.path(), &|_| None));
        assert!(unreadable.is_empty());
        (dir, store)
    }

    #[test]
    fn an_ini_flag_is_stored_as_one_or_zero() {
        let (_dir, mut store) = fresh();
        assert!(store.set("vsync", json!(false)));
        assert_eq!(store.ini().values["vsync"], "0");
        assert!(!store.flag("vsync"));
        assert!(store.set("vsync", json!(true)));
        assert_eq!(store.ini().values["vsync"], "1");
        assert!(!store.set("vsync", json!(true))); // unchanged
        assert!(store.app().get("vsync").is_none());
    }

    #[test]
    fn an_unknown_key_goes_to_settings_json() {
        let (_dir, mut store) = fresh();
        assert!(store.set("my_new_key", json!("x")));
        assert_eq!(store.app().get("my_new_key"), Some(&json!("x")));
        assert_eq!(store.json("my_new_key"), Some(&json!("x")));
        assert!(!store.ini().values.contains_key("my_new_key"));
        assert!(!store.set("my_new_key", json!("x")));
    }

    #[test]
    fn replace_names_the_changed_keys() {
        let (_dir, mut store) = fresh();
        let mut app = store.app().clone();
        app.set("game_dir", json!("/games"));
        let mut ini = Ini {
            values: store.ini().values.clone(),
            lines: Vec::new(),
        };
        ini.values.insert("engine_fps".into(), "60".into());
        assert_eq!(store.replace(app, ini), vec!["game_dir", "engine_fps"]);
        assert_eq!(store.text("game_dir"), "/games");
        assert_eq!(store.text("engine_fps"), "60");
    }

    #[test]
    fn ini_to_write_normalizes_only_shown_flags() {
        let (_dir, mut store) = fresh();
        // Only "1" is on in an ini: "true" is written as off, a bare "1" stays on.
        store.set("vsync", json!("true"));
        store.set("show_fps", json!("1"));
        store.set("upscaler", json!("xx"));
        store.set("fps_limit", json!("60"));
        let out = store.ini_to_write();
        assert_eq!(out["vsync"], "0");
        assert_eq!(out["show_fps"], "1");
        assert_eq!(out["fps_limit"], "60");
        assert!(!out.contains_key("upscaler"));
        assert_eq!(out.len(), 18);
        assert_eq!(out.keys().next().unwrap(), "display_mode");
    }

    #[test]
    fn an_unreadable_file_is_listed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_env(dir.path(), &|_| None);
        std::fs::write(paths.config_file(), "{not json").unwrap();
        let (_store, unreadable) = Store::load(&paths);
        assert_eq!(unreadable, vec![paths.config_file()]);
    }
}
