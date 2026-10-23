//! The runner (Python `run.py`): starts the game under the probe and reports what it saw.

pub mod content_profile;
pub mod draw;
pub mod dxt;
pub mod game_update;
pub mod glyphs;
pub mod graphics_options;
pub mod guest_memory;
pub mod isa_rewrite;
pub mod link_libc;
pub mod link_modules;
pub mod pause_menu;
pub mod prepare;
pub mod window_icon;

/// The synthetic probe inputs the runner's tests build (`tests/test_probe.py::package` and more).
#[cfg(test)]
#[path = "../../tests/common/packages.rs"]
mod packages;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::paths::Paths;
use crate::core::platform;

use indexmap::IndexMap;

/// What one run needs from the program around it.
pub struct RunContext<'a> {
    pub port_dir: &'a Path,
    pub env: IndexMap<String, String>,
    pub args: Vec<OsString>,
    pub out: &'a mut dyn Write,
}

/// Runs the runner and returns its exit code.
pub fn run(context: RunContext<'_>) -> i32 {
    let probe = if cfg!(windows) {
        "spartanport-probe.exe"
    } else {
        "spartanport-probe"
    };
    // A closed stdout is nothing to report on: the code says it.
    let _ = writeln!(
        context.out,
        "{probe} not found: build it with build.sh or use a packaged build."
    );
    1
}

/// Instruction sets `spartanport-probe --cpu-features` reports missing (empty when complete).
pub fn parse_cpu_features(output: &str) -> Vec<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("CPU: missing "))
        .map(|names| {
            names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `SPARTAN_ISA_REWRITE=1` forces the ISA rewrite table, `0` disables it; otherwise the CPU decides.
pub fn isa_rewrite_wanted(missing: &[String], env: &dyn Fn(&str) -> Option<String>) -> bool {
    match env("SPARTAN_ISA_REWRITE").as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => !missing.is_empty(),
    }
}

/// The command that starts the game on Linux when GameMode is chosen in the launcher (empty on
/// Windows, without the choice, or when `gamemoderun` is not on `PATH`).
pub fn launch_prefix(env: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    if cfg!(windows) || env("SPARTAN_GAMEMODE").as_deref() != Some("1") {
        return Vec::new();
    }
    match env("PATH").and_then(|path| which("gamemoderun", &path)) {
        Some(found) => vec![found.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

/// The first executable file called `name` in the directories of `path`.
fn which(name: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// The probe: `SPARTAN_PROBE` when set (taken as is, like `run.py`), else `bin/` or `out/` of the port.
pub fn find_executable(paths: &Paths, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env("SPARTAN_PROBE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| paths.find_tool("spartanport-probe"))
}

/// Runs a command within a time limit: its output, or the error to print.
pub type ProbeRunner<'a> = dyn Fn(&[&str], Duration) -> Result<String, String> + 'a;

/// How long `glyph_mounts` waits for `spartanport-probe --gamepad`.
pub const GAMEPAD_PROBE_LIMIT: Duration = Duration::from_secs(15);

/// The probe arguments and style of the controller glyphs (`run.py` `glyph_mounts`): patched copies
/// of the WADs that hold the game's button icons, mounted over them (`glyphs::build`). Opt-in:
/// unless `SPARTAN_GLYPH_STYLE` is auto, xbox, nintendo or keyboard nothing is asked, drawn or
/// mounted, and old copies are removed. The keyboard keys follow `SPARTAN_KEY_MAP`; auto shows them
/// when no gamepad is connected. Whatever fails only reaches the Log: the game then keeps its own
/// icons. `run_probe` runs a command (`[probe, --gamepad]`) within a time limit and returns its
/// output or the error to print.
pub fn glyph_mounts(
    env: &IndexMap<String, String>,
    probe: &Path,
    game: &Path,
    out: &Path,
    assets: &glyphs::Assets,
    run_probe: &ProbeRunner<'_>,
    log: &mut dyn FnMut(String),
) -> (Vec<String>, Option<String>) {
    let get = |key: &str| env.get(key).map(String::as_str).unwrap_or("");
    let choice = env
        .get("SPARTAN_GLYPH_STYLE")
        .map(String::as_str)
        .unwrap_or("off");
    if !["auto", "xbox", "nintendo", "keyboard"].contains(&choice) {
        let _ = std::fs::remove_dir_all(out.join("glyphs"));
        return (Vec::new(), None);
    }
    let failed = |log: &mut dyn FnMut(String), error: &dyn std::fmt::Display| {
        log(format!(
            "Glyphs: {error}; the game keeps its own button icons"
        ));
        (Vec::new(), None)
    };
    let pad = if choice == "auto" {
        let program = probe.to_string_lossy();
        match run_probe(&[&program, "--gamepad"], GAMEPAD_PROBE_LIMIT) {
            Ok(output) => output,
            Err(error) => return failed(log, &error),
        }
    } else {
        String::new()
    };
    let resolved = glyphs::resolve(choice, &pad);
    let mounts = match glyphs::build(
        game,
        out,
        &resolved.style,
        resolved.faces.as_ref(),
        get("SPARTAN_PAD_MAP"),
        get("SPARTAN_KEY_MAP"),
        assets,
        log,
    ) {
        Ok(mounts) => mounts,
        Err(error) => return failed(log, &error),
    };
    if mounts.is_empty() {
        log(if resolved.style == "playstation" {
            "Glyphs: a PlayStation controller, the game keeps its own button icons".to_string()
        } else {
            "Glyphs: no button icons found in this game folder".to_string()
        });
        return (Vec::new(), None);
    }
    let name = if resolved.name.is_empty() {
        String::new()
    } else {
        format!(" ({})", resolved.name)
    };
    log(format!(
        "Glyphs: {}{name}, on copies in {}; game files untouched",
        resolved.style,
        out.join("glyphs").display()
    ));
    let arguments = mounts
        .iter()
        .flat_map(|(guest, path)| ["--mount".to_string(), format!("{guest}={}", path.display())])
        .collect();
    (arguments, Some(resolved.style))
}

/// Probe arguments for the copy of `R_UI.WAD` with the pause menu's Exit to Desktop row
/// (`pause_menu::build`), made from the glyphs' copy when `mounts` (the probe arguments of
/// [`glyph_mounts`]) has one (`run.py` `pause_menu_args`). The probe mounts it over both once its
/// hook is in (`src/menu_hook.h`). The game folder is never written; what fails only reaches the
/// Log, and the game then starts without the row.
pub fn pause_menu_args(
    game: &Path,
    out: &Path,
    mounts: &[String],
    language: Option<&str>,
    log: &mut dyn FnMut(String),
) -> Vec<String> {
    let guest = format!("/app0/{}=", pause_menu::WAD);
    let source = mounts
        .iter()
        .find_map(|part| part.strip_prefix(guest.as_str()))
        .map(Path::new);
    match pause_menu::build(game, out, source, language, log) {
        Ok(made) => made
            .into_iter()
            .flat_map(|path| ["--pause-menu".to_string(), pause_menu::python_path(&path)])
            .collect(),
        Err(error) => {
            log(format!("Pause menu: {error}; no Exit to Desktop row"));
            Vec::new()
        }
    }
}

/// Waits up to `limit` for process `pid` to end (the previous game, for `--after`). `true` when it
/// ended or never existed.
pub fn wait_for_pid(pid: u32, limit: Duration) -> bool {
    platform::wait_for_process(pid, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn parse_cpu_features_reads_the_missing_line() {
        assert_eq!(
            parse_cpu_features("CPU: missing MOVBE, BMI1, LZCNT\n"),
            ["MOVBE", "BMI1", "LZCNT"]
        );
        assert!(parse_cpu_features("CPU: complete\n").is_empty());
    }

    #[test]
    fn rewrite_wanted_follows_cpu_by_default() {
        assert!(isa_rewrite_wanted(&["MOVBE".to_string()], &env_of(&[])));
        assert!(!isa_rewrite_wanted(&[], &env_of(&[])));
    }

    #[test]
    fn rewrite_forced() {
        assert!(isa_rewrite_wanted(
            &[],
            &env_of(&[("SPARTAN_ISA_REWRITE", "1")])
        ));
    }

    #[test]
    fn rewrite_disabled() {
        let missing = ["MOVBE".to_string(), "BMI1".to_string()];
        assert!(!isa_rewrite_wanted(
            &missing,
            &env_of(&[("SPARTAN_ISA_REWRITE", "0")])
        ));
    }

    #[test]
    fn launch_prefix_is_empty_without_gamemode() {
        assert!(launch_prefix(&env_of(&[])).is_empty());
    }

    // -- RunGlyphs (tests/test_glyphs.py): the option is off unless SPARTAN_GLYPH_STYLE asks ----------

    struct GlyphRun {
        _dir: tempfile::TempDir,
        game: PathBuf,
        out: PathBuf,
    }

    /// `out/glyphs/R_UI.WAD` left from an earlier run, and a game folder holding the synthetic
    /// `R_UI.WAD` of the glyph tests when `with_wad`.
    fn glyph_run(with_wad: bool) -> GlyphRun {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(out.join("glyphs")).unwrap();
        std::fs::write(out.join("glyphs/R_UI.WAD"), b"old").unwrap();
        let game = dir.path().join("game");
        std::fs::create_dir_all(game.join("EXEC/WAD/PS4_LE")).unwrap();
        if with_wad {
            std::fs::write(game.join("EXEC/WAD/PS4_LE/R_UI.WAD"), glyphs::tests::wad()).unwrap();
        }
        GlyphRun {
            _dir: dir,
            game,
            out,
        }
    }

    fn glyph_assets() -> glyphs::Assets {
        glyphs::Assets::in_port(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/..")))
    }

    fn env_map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// What `glyph_mounts` returned, the probe calls and the Log lines.
    type GlyphOutcome = (
        (Vec<String>, Option<String>),
        Vec<(Vec<String>, Duration)>,
        Vec<String>,
    );

    /// Runs `glyph_mounts` with `probe` standing in for `spartanport-probe --gamepad`.
    fn mounts_with(
        run: &GlyphRun,
        env: &[(&str, &str)],
        probe: Result<&str, &str>,
    ) -> GlyphOutcome {
        let calls = std::cell::RefCell::new(Vec::new());
        let answer = |args: &[&str], limit: Duration| {
            calls
                .borrow_mut()
                .push((args.iter().map(|a| a.to_string()).collect(), limit));
            probe.map(str::to_string).map_err(str::to_string)
        };
        let mut log = Vec::new();
        let result = glyph_mounts(
            &env_map(env),
            Path::new("probe"),
            &run.game,
            &run.out,
            &glyph_assets(),
            &answer,
            &mut |line| log.push(line),
        );
        (result, calls.into_inner(), log)
    }

    fn stamp(run: &GlyphRun) -> String {
        std::fs::read_to_string(run.out.join("glyphs/stamp.json")).unwrap()
    }

    #[test]
    fn off_runs_nothing_and_removes_old_copies() {
        let run = glyph_run(true);
        for env in [
            vec![],
            vec![("SPARTAN_GLYPH_STYLE", "off")],
            vec![("SPARTAN_GLYPH_STYLE", "playstation")],
        ] {
            std::fs::create_dir_all(run.out.join("glyphs")).unwrap();
            let (result, calls, _) = mounts_with(&run, &env, Ok(""));
            assert_eq!(result, (vec![], None));
            assert!(calls.is_empty());
            assert!(!run.out.join("glyphs").exists());
        }
    }

    #[test]
    fn auto_asks_the_probe_and_mounts_the_copies() {
        let run = glyph_run(true);
        let pad = "Gamepad: type=xboxone south=a east=b west=x north=y name=Xbox Controller\n";
        let env = [
            ("SPARTAN_GLYPH_STYLE", "auto"),
            ("SPARTAN_PAD_MAP", "cross=b"),
        ];
        let ((arguments, style), calls, log) = mounts_with(&run, &env, Ok(pad));
        assert_eq!(
            calls,
            [(
                vec!["probe".to_string(), "--gamepad".to_string()],
                Duration::from_secs(15)
            )]
        );
        // glyphs::build got the controller's style and letters and the gamepad map, no key map.
        let faces = r#"{"south": "a", "east": "b", "west": "x", "north": "y"}"#;
        assert!(
            stamp(&run).starts_with(&format!("[11, \"xbox\", {faces}, \"cross=b\", 1")),
            "{}",
            stamp(&run)
        );
        let copy = run.out.join("glyphs").join("R_UI.WAD");
        assert_eq!(
            arguments,
            [
                "--mount".to_string(),
                format!("/app0/EXEC/WAD/PS4_LE/R_UI.WAD={}", copy.display())
            ]
        );
        assert_eq!(style.as_deref(), Some("xbox"));
        let folder = run.out.join("glyphs");
        assert_eq!(
            log,
            [format!(
                "Glyphs: xbox (Xbox Controller), on copies in {}; game files untouched",
                folder.display()
            )]
        );
    }

    #[test]
    fn a_chosen_style_does_not_ask_the_probe() {
        let run = glyph_run(false);
        let (result, calls, log) =
            mounts_with(&run, &[("SPARTAN_GLYPH_STYLE", "nintendo")], Ok(""));
        assert_eq!(result, (vec![], None));
        assert!(calls.is_empty());
        assert!(
            stamp(&run).starts_with("[11, \"nintendo\", null, \"\", 1"),
            "{}",
            stamp(&run)
        );
        assert_eq!(log, ["Glyphs: no button icons found in this game folder"]);
    }

    #[test]
    fn keyboard_passes_the_key_map() {
        let run = glyph_run(true);
        let env = [
            ("SPARTAN_GLYPH_STYLE", "keyboard"),
            ("SPARTAN_KEY_MAP", "cross=Q"),
        ];
        let ((_, style), calls, _) = mounts_with(&run, &env, Ok(""));
        assert_eq!(style.as_deref(), Some("keyboard"));
        assert!(calls.is_empty());
        assert!(
            stamp(&run).starts_with("[11, \"keyboard\", null, \"\", \"cross=Q\", null, "),
            "{}",
            stamp(&run)
        );
    }

    #[test]
    fn auto_without_a_gamepad_shows_the_keys() {
        let run = glyph_run(true);
        let env = [
            ("SPARTAN_GLYPH_STYLE", "auto"),
            ("SPARTAN_KEY_MAP", "square=G"),
        ];
        let ((_, style), _, _) = mounts_with(&run, &env, Ok("Gamepad: none\n"));
        assert_eq!(style.as_deref(), Some("keyboard"));
        assert!(
            stamp(&run).starts_with("[11, \"keyboard\", null, \"\", \"square=G\", null, "),
            "{}",
            stamp(&run)
        );
    }

    #[test]
    fn a_failure_starts_the_game_with_its_own_glyphs() {
        let run = glyph_run(true);
        let (result, _, log) =
            mounts_with(&run, &[("SPARTAN_GLYPH_STYLE", "auto")], Err("no probe"));
        assert_eq!(result, (vec![], None));
        assert_eq!(
            log,
            ["Glyphs: no probe; the game keeps its own button icons"]
        );
        // glyphs.build failing (here: out/ is a file, so out/glyphs cannot be made).
        let blocked = GlyphRun {
            _dir: tempfile::tempdir().unwrap(),
            game: run.game.clone(),
            out: run.out.join("glyphs").join("R_UI.WAD"),
        };
        let (result, _, log) = mounts_with(&blocked, &[("SPARTAN_GLYPH_STYLE", "xbox")], Ok(""));
        assert_eq!(result, (vec![], None));
        assert!(
            log.len() == 1
                && log[0].starts_with("Glyphs: ")
                && log[0].ends_with("; the game keeps its own button icons"),
            "{log:?}"
        );
    }

    // -- pause_menu_args (run.py): the copy with the Exit to Desktop row ---------------------------

    /// What `pause_menu_args` returned and the Log lines.
    fn menu_args(game: &Path, out: &Path, mounts: &[String]) -> (Vec<String>, Vec<String>) {
        let mut log = Vec::new();
        let args = pause_menu_args(game, out, mounts, Some("en"), &mut |line| log.push(line));
        (args, log)
    }

    #[test]
    fn pause_menu_args_patch_the_glyph_copy_when_it_is_mounted() {
        // The game folder has no R_UI.WAD: only the glyph copy can be the source.
        let run = glyph_run(false);
        let copy = run.out.join("glyphs").join("R_UI.WAD");
        std::fs::write(&copy, glyphs::tests::wad()).unwrap();
        let mounts = [
            "--mount".to_string(),
            "/app0/EXEC/WAD/PS4_LE/R_UI.WAD.bak=elsewhere".to_string(),
            "--mount".to_string(),
            format!("/app0/EXEC/WAD/PS4_LE/R_UI.WAD={}", copy.display()),
        ];
        let (args, log) = menu_args(&run.game, &run.out, &mounts);
        assert!(args.is_empty());
        assert_eq!(
            log,
            ["Pause menu: no SMI_R_UI in this WAD; no Exit to Desktop row"]
        );
        // Without the mount the game's own R_UI.WAD is the source; there is none here.
        let (args, log) = menu_args(&run.game, &run.out, &[]);
        assert!(args.is_empty());
        assert!(log.is_empty(), "{log:?}");
    }

    #[test]
    fn pause_menu_args_report_what_fails_and_start_without_the_row() {
        let run = glyph_run(true);
        // out/ is a file, so out/pause_menu cannot be made.
        let out = run.out.join("glyphs").join("R_UI.WAD");
        let (args, log) = menu_args(&run.game, &out, &[]);
        assert!(args.is_empty());
        assert!(
            log.len() == 1
                && log[0].starts_with("Pause menu: ")
                && log[0].ends_with("; no Exit to Desktop row"),
            "{log:?}"
        );
    }

    #[test]
    fn find_executable_prefers_the_probe_variable() {
        let dir = tempfile::tempdir().unwrap();
        let probe = dir.path().join("my-probe");
        std::fs::write(&probe, b"").unwrap();
        let paths = Paths::from_env(dir.path(), &|_| None);
        let env = env_of(&[("SPARTAN_PROBE", probe.to_str().unwrap_or(""))]);
        assert_eq!(find_executable(&paths, &env), Some(probe));
        assert_eq!(find_executable(&paths, &env_of(&[])), None);
    }
}
