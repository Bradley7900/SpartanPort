#![allow(clippy::unwrap_used, clippy::expect_used)]

use spartanport_launcher::core::legacy::{LEGACY_EXE, LEGACY_PLAY_EXE};

fn read(relative: &str) -> String {
    let path = format!("{}/../{relative}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn the_windows_package_ships_the_rust_launcher() {
    let package = read("packaging/windows/package.sh");
    for needed in [
        "launcher-rust/target/release/spartanport-launcher.exe",
        "legacy::LEGACY_EXE",
        "legacy::LEGACY_PLAY_EXE",
        LEGACY_EXE,
        LEGACY_PLAY_EXE,
        "scripts/controller_diagrams",
        "-iname '*.py'",
        "-iname _internal",
    ] {
        assert!(package.contains(needed), "package.sh lacks {needed}");
    }
    for banned in ["PyInstaller", "pyenv", "run.py", "cp -r scripts patches"] {
        assert!(!package.contains(banned), "package.sh still has {banned}");
    }

    let smoke = read("packaging/windows/smoke-test.ps1");
    for needed in ["--version", "--voice-swap"] {
        assert!(smoke.contains(needed), "smoke-test.ps1 lacks {needed}");
    }
    assert!(
        !smoke.contains("--script"),
        "smoke-test.ps1 still has --script"
    );

    let workflow = read(".github/workflows/windows-zip.yml");
    for needed in [
        "dtolnay/rust-toolchain@stable",
        "cargo build --release --locked",
    ] {
        assert!(workflow.contains(needed), "windows-zip.yml lacks {needed}");
    }
}
