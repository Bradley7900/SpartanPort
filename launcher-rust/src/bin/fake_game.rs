//! A stand-in for the game used by the launch tests.
//!
//! `fake-game --lines N --delay-ms D --exit CODE [--silent-after N]` prints `line K` for K in
//! 1..=N, sleeping D ms between lines. With `--silent-after N` it stops printing after N lines
//! and keeps running for 500 ms before exiting, so the exit comes well after the last output.

use std::io::Write;
use std::time::Duration;

struct Args {
    lines: u32,
    delay_ms: u64,
    exit: i32,
    silent_after: Option<u32>,
}

fn parse() -> Option<Args> {
    let mut args = Args {
        lines: 0,
        delay_ms: 0,
        exit: 0,
        silent_after: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next()?;
        match flag.as_str() {
            "--lines" => args.lines = value.parse().ok()?,
            "--delay-ms" => args.delay_ms = value.parse().ok()?,
            "--exit" => args.exit = value.parse().ok()?,
            "--silent-after" => args.silent_after = Some(value.parse().ok()?),
            _ => return None,
        }
    }
    Some(args)
}

fn main() {
    let Some(args) = parse() else {
        eprintln!("usage: fake-game --lines N --delay-ms D --exit CODE [--silent-after N]");
        std::process::exit(2);
    };
    let printed = args.silent_after.map_or(args.lines, |n| n.min(args.lines));
    let mut out = std::io::stdout();
    for k in 1..=args.lines {
        if k <= printed {
            let _ = writeln!(out, "line {k}");
            let _ = out.flush();
        }
        if args.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(args.delay_ms));
        }
    }
    if args.silent_after.is_some() {
        std::thread::sleep(Duration::from_millis(500));
    }
    std::process::exit(args.exit);
}
