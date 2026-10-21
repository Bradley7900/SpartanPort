#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Parity tests: the Rust core must reproduce what the Python launcher wrote into `fixtures/`.

use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use spartanport_launcher::core::ini::{load_ini, save_ini};
use spartanport_launcher::core::settings::{
    app_defaults, migrate_controls, save_settings, Settings,
};

fn fixture_dir(case: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures")).join(case)
}

fn load_input(case: &str) -> serde_json::Value {
    let text = fs::read_to_string(fixture_dir(case).join("input.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn ini_matches_python_for_case_edited() {
    let input = load_input("edited");
    let mut text = String::new();
    for line in input["ini_lines"].as_array().unwrap() {
        text.push_str(line.as_str().unwrap());
        text.push('\n');
    }
    let edits: IndexMap<String, String> =
        serde_json::from_value(input["edits"]["ini"].clone()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spartanport.ini");
    fs::write(&path, text).unwrap();
    let ini = load_ini(&path);
    save_ini(&path, &edits, &ini.lines).unwrap();

    let expected = fs::read(fixture_dir("edited").join("spartanport.ini")).unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected);
}

fn settings_bytes_for_case(case: &str) -> Vec<u8> {
    let input = load_input(case);
    let env = |key: &str| input["env"][key].as_str().map(str::to_string);
    let mut merged = app_defaults(Path::new("<port>"), true, &env);
    let saved = input["settings"].as_object().unwrap().clone();
    for (key, value) in migrate_controls(saved) {
        merged.insert(key, value);
    }
    let mut settings = Settings(merged);
    for (key, value) in input["edits"]["app"].as_object().unwrap() {
        settings.set(key, value.clone());
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    save_settings(&path, &settings).unwrap();
    fs::read(&path).unwrap()
}

#[test]
fn settings_match_python_for_case_defaults() {
    let expected = fs::read(fixture_dir("defaults").join("settings.json")).unwrap();
    assert_eq!(settings_bytes_for_case("defaults"), expected);
}

#[test]
fn settings_match_python_for_case_edited() {
    let expected = fs::read(fixture_dir("edited").join("settings.json")).unwrap();
    assert_eq!(settings_bytes_for_case("edited"), expected);
}
