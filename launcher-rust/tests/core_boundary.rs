#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn walk(root: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn core_and_runner_never_mention_iced() {
    for root in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/core"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/runner"),
    ] {
        for entry in walk(root) {
            let text = std::fs::read_to_string(&entry).unwrap();
            assert!(!text.contains("iced"), "{entry:?} imports iced");
        }
    }
}

/// The `VERSION = '…'` line of the Python launcher in this checkout.
fn python_version() -> String {
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../launcher/spartanport_launcher_win.py"
    );
    let text = std::fs::read_to_string(source).unwrap();
    text.lines()
        .find_map(|line| line.strip_prefix("VERSION = '"))
        .and_then(|rest| rest.strip_suffix('\''))
        .unwrap()
        .to_string()
}

#[test]
fn version_is_the_python_launchers() {
    assert_eq!(spartanport_launcher::VERSION, python_version());
    // The crate's own version (the stamp and the exe's metadata) is the same one.
    assert_eq!(env!("CARGO_PKG_VERSION"), spartanport_launcher::VERSION);
}
