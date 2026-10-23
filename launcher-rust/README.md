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

- `SPARTAN_PORT_DIR`: the port folder (with `run.py`, `bin/` and `scripts/`) and, unless `SPARTAN_DATA_DIR` is set, the data folder. Defaults to the executable's folder; point it at the repository root to run a `cargo` build.
- `SPARTANPORT_PYTHON`: path of the Python interpreter used for the commands still forwarded to the Python launcher; PLAY no longer needs it (the exe runs the game itself with `--run`). Defaults to `py -3` on Windows and `python3` elsewhere.
