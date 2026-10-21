use std::path::{Path, PathBuf};

use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::{ui, APP_NAME, VERSION};

fn main() {
    if std::env::args().skip(1).any(|arg| arg == "--version") {
        println!("{APP_NAME} {VERSION}");
        return;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = Paths::from_env(&exe_dir, &|key| std::env::var(key).ok());
    if let Err(error) = ui::app::run(paths) {
        eprintln!("{APP_NAME}: {error}");
        std::process::exit(1);
    }
}
