use spartanport_launcher::{APP_NAME, VERSION};

fn main() {
    if std::env::args().skip(1).any(|arg| arg == "--version") {
        println!("{APP_NAME} {VERSION}");
    } else {
        println!("GUI not built yet");
    }
    std::process::exit(0);
}
