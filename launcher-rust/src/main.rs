use std::ffi::OsString;
use std::path::{Path, PathBuf};

use spartanport_launcher::core::paths::Paths;
use spartanport_launcher::startup::{self, Route};
use spartanport_launcher::{ui, APP_NAME, VERSION};

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    match startup::route(&args) {
        Route::Version => println!("{APP_NAME} {VERSION}"),
        Route::Python => std::process::exit(startup::forward_to_python(&exe_dir, &args)),
        Route::Launcher => {
            // Before the folders below are read from the environment.
            startup::forward_environment();
            let paths = Paths::from_env(&exe_dir, &|key| std::env::var(key).ok());
            startup::prepare(&paths);
            if let Err(error) = ui::app::run(paths) {
                eprintln!("{APP_NAME}: {error}");
                std::process::exit(1);
            }
        }
    }
}
