#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Parity tests: the runner must write what the Python modules wrote into `fixtures/runner/`
//! (`tools/runner_fixtures.py`).

use std::fs;
use std::path::{Path, PathBuf};

use spartanport_launcher::runner::link_modules::DEFAULT_MODULES;
use spartanport_launcher::runner::prepare::prepare;
use spartanport_launcher::runner::{link_libc, link_modules};

fn fixture_dir(case: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/runner")).join(case)
}

/// The bytes of `expected/<name>` in `case`.
fn expected(case: &str, name: &str) -> Vec<u8> {
    fs::read(fixture_dir(case).join("expected").join(name)).unwrap()
}

/// `text` with the temporary folder `dir` spelled `<DATA_DIR>`, as the generator wrote it.
fn with_placeholder(text: &str, dir: &Path) -> String {
    let plain = dir.display().to_string();
    let escaped = plain.replace('\\', "\\\\");
    text.replace(&escaped, "<DATA_DIR>")
        .replace(&plain, "<DATA_DIR>")
}

/// Fails at the first differing byte instead of printing both files.
#[track_caller]
fn assert_same_bytes(actual: &[u8], expected: &[u8], name: &str) {
    if let Some(at) = actual.iter().zip(expected).position(|(a, b)| a != b) {
        panic!(
            "{name}: byte {at:#x} is {:#04x}, the Python wrote {:#04x}",
            actual[at], expected[at]
        );
    }
    assert_eq!(actual.len(), expected.len(), "{name}: length");
}

#[test]
fn prepare_matches_python() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    prepare(&fixture_dir("prepare").join("input"), &out).unwrap();
    for name in ["boot.bin", "eboot.elf", "entry.bin"] {
        let actual = fs::read(out.join(name)).unwrap();
        assert_same_bytes(&actual, &expected("prepare", name), name);
    }
    let analysis = String::from_utf8(fs::read(out.join("analysis.json")).unwrap()).unwrap();
    let expected = String::from_utf8(expected("prepare", "analysis.json")).unwrap();
    assert_eq!(with_placeholder(&analysis, dir.path()), expected);
}

#[test]
fn link_matches_python() {
    let dir = tempfile::tempdir().unwrap();
    let game = fixture_dir("link").join("input");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).unwrap();
    fs::copy(game.join("boot.bin"), out.join("boot.bin")).unwrap();
    link_libc::link(&game, &out).unwrap();
    link_modules::link(&game, &out, &DEFAULT_MODULES).unwrap();
    for name in ["boot-libc.bin", "boot-linked.bin"] {
        let actual = fs::read(out.join(name)).unwrap();
        assert_same_bytes(&actual, &expected("link", name), name);
    }
    for name in ["libc-link.json", "link.json"] {
        let actual = String::from_utf8(fs::read(out.join(name)).unwrap()).unwrap();
        let expected = String::from_utf8(expected("link", name)).unwrap();
        assert_eq!(with_placeholder(&actual, dir.path()), expected, "{name}");
    }
}
