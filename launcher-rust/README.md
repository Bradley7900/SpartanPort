# SpartanPort launcher (Rust)

Rust rewrite of the Python/Tk launcher. `src/core/` holds the launcher logic and never depends on the UI crate; `src/ui/` holds the graphical front end.

## Toolchain

Install Rust with [rustup](https://rustup.rs). The stable channel is selected by `rust-toolchain.toml`.

- Windows: install Visual Studio Build Tools with the "Desktop development with C++" workload.
- Ubuntu: `sudo apt-get install build-essential pkg-config libwayland-dev libxkbcommon-dev`

## Tests

```
cd launcher-rust
cargo test
```

## Environment

- `SPARTANPORT_PYTHON`: path of the Python interpreter used to start `run.py` until the Rust runner exists. Defaults to `py -3` on Windows and `python3` elsewhere.
