use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use spartanport_launcher::runner::wait_for_pid;

#[test]
fn wait_for_pid_returns_when_the_process_ends() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-game"))
        .args(["--exit", "0", "--lines", "1", "--delay-ms", "300"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    assert!(wait_for_pid(child.id(), Duration::from_secs(10)));
    assert!(started.elapsed() < Duration::from_secs(10));
    child.wait().unwrap();
    // A pid that never existed: true at once.
    let started = Instant::now();
    assert!(wait_for_pid(u32::MAX - 1, Duration::from_secs(10)));
    assert!(started.elapsed() < Duration::from_secs(2));
}
