// A windowed program on Windows: no console opens with the launcher. From a terminal it attaches
// to the terminal's console (`platform::attach_parent_console`).
#![windows_subsystem = "windows"]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use spartanport_launcher::core::env::environment_of;
use spartanport_launcher::core::paths::{default_port_dir, Paths};
use spartanport_launcher::core::platform;
use spartanport_launcher::runner::{self, RunContext};
use spartanport_launcher::startup::{self, Route};
use spartanport_launcher::{ui, APP_NAME, VERSION};

fn main() {
    // First, so `--version`, the forwarded Python and start-up errors print in a terminal.
    platform::attach_parent_console();
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    // A packaged port, or the checkout a cargo build sits in (`launcher-rust/target/…`).
    let port_dir = default_port_dir(&exe_dir);
    match startup::route(&args) {
        Route::Version => println!("{APP_NAME} {VERSION}"),
        // `Route::Play` routes like `Run` until the play role exists.
        Route::Run | Route::Play => {
            let paths = Paths::from_env(&port_dir, &|key| std::env::var(key).ok());
            let context = RunContext {
                port_dir: &paths.port_dir,
                env: environment_of(std::env::vars_os()),
                args: args.iter().skip(1).cloned().collect(),
                out: &mut std::io::stdout(),
            };
            std::process::exit(runner::run(context))
        }
        Route::Python => {
            // The Python launcher of the port folder (`SPARTAN_PORT_DIR` included).
            let paths = Paths::from_env(&port_dir, &|key| std::env::var(key).ok());
            std::process::exit(startup::forward_to_python(&paths.port_dir, &args))
        }
        Route::Launcher => {
            // Before the folders below are read from the environment.
            startup::forward_environment();
            let paths = Paths::from_env(&port_dir, &|key| std::env::var(key).ok());
            startup::prepare(&paths);
            if let Err(error) = ui::app::run(paths) {
                eprintln!("{APP_NAME}: {error}");
                std::process::exit(1);
            }
        }
    }
}
