//! The Advanced page: launcher options, Linux integrations, files, the shader cache and the
//! diagnostics. The Log is its own page.
//!
//! `view` only draws. The slow work (the shortcut, the Steam bridge, the cache removal) runs on a
//! thread and comes back as a message; the Linux tools were probed once in `State::new`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use iced::widget::{button, checkbox as check_box, column, row as layout, text};
use iced::{Center, Element, Task};
use serde_json::Value;

use crate::core::desktop::{host_environment, windows_shortcut, write_linux_shortcut};
use crate::core::i18n::{tr, tr_fmt};
use crate::core::launch::python_command;
use crate::core::options::{UI_LANGUAGES, UI_SCALES, VIDEO_BACKENDS};
use crate::core::paths::Paths;
use crate::core::settings::Settings;
use crate::ui::app::{Message, State};
use crate::ui::pages::performance::SelectRow;
use crate::ui::persist;
use crate::ui::play::GameState;
use crate::ui::tasks::blocking;
use crate::ui::theme::{self, CINZEL_BOLD};
use crate::ui::update::UpdateMessage;
use crate::ui::widgets::{checkbox, choice_pairs, row, section, select_pairs, Modal, ModalKind};
use crate::{APP_NAME, VERSION};

/// One check box of the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRow {
    pub key: &'static str,
    pub label: String,
    pub hint: String,
    pub checked: bool,
    /// False for a tool this system does not have.
    pub enabled: bool,
}

/// Whether the Home fire burns: on unless `launcher_effects` is explicitly off.
pub fn effects_enabled(settings: &Settings) -> bool {
    match settings.get("launcher_effects") {
        Some(Value::Bool(on)) => *on,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => s != "0" && s != "false",
        _ => true,
    }
}

/// Whether the game runs, which blocks clearing its shader cache.
pub fn clear_cache_blocked(state: &State) -> bool {
    // As `if self.process: return` does.
    match state.game {
        GameState::Idle | GameState::Copying => false,
        GameState::Preparing { .. } | GameState::Running { .. } => true,
    }
}

fn restart(lang: &str, english: &str) -> String {
    format!("{}  ·  {}", tr(lang, english), tr(lang, "restart"))
}

/// The drop-downs of the page: the language, the scale and (off Windows) the window system.
pub fn rows(state: &State, lang: &str) -> Vec<SelectRow> {
    let app = |key: &str| state.settings.get_str(key);
    let mut rows = vec![
        SelectRow {
            key: "ui_language",
            pairs: choice_pairs(UI_LANGUAGES, lang),
            current: app("ui_language"),
        },
        SelectRow {
            key: "ui_scale",
            pairs: choice_pairs(UI_SCALES, lang),
            current: app("ui_scale"),
        },
    ];
    if !cfg!(windows) {
        rows.push(SelectRow {
            key: "video_backend",
            pairs: choice_pairs(VIDEO_BACKENDS, lang),
            current: app("video_backend"),
        });
    }
    rows
}

fn check_row(
    state: &State,
    key: &'static str,
    label: String,
    hint: String,
    enabled: bool,
) -> CheckRow {
    CheckRow {
        key,
        label,
        hint,
        checked: if key == "launcher_effects" {
            effects_enabled(&state.settings)
        } else {
            state.settings.get_bool(key)
        },
        enabled,
    }
}

/// The check boxes of the Launcher section, the Effects row last.
pub fn launcher_checks(state: &State, lang: &str) -> Vec<CheckRow> {
    let mut checks = vec![
        check_row(
            state,
            "discord_rpc",
            tr(lang, "Show Discord Rich Presence"),
            tr(
                lang,
                "Shows only the game name and the time played on your Discord profile.",
            ),
            true,
        ),
        check_row(
            state,
            "close_on_play",
            tr(lang, "Close the launcher when the game starts"),
            String::new(),
            true,
        ),
    ];
    if state.self_update {
        checks.push(check_row(
            state,
            "check_updates",
            tr(lang, "Check for updates when the launcher opens"),
            String::new(),
            true,
        ));
    }
    checks.push(check_row(
        state,
        "launcher_effects",
        tr(lang, "Effects"),
        tr(
            lang,
            "Embers and flames on the Home screen. Off saves GPU time.",
        ),
        true,
    ));
    checks
}

/// The check boxes of the Linux section; empty on Windows.
pub fn linux_checks(state: &State, lang: &str) -> Vec<CheckRow> {
    if cfg!(windows) {
        return Vec::new();
    }
    let tools = state.linux_tools;
    let missing = |present: bool| {
        if present {
            String::new()
        } else {
            format!("  {}", tr(lang, "Not installed."))
        }
    };
    vec![
        check_row(
            state,
            "desktop_integration",
            tr(lang, "Menu entry and icon (AppImage)"),
            tr(
                lang,
                "Creates spartanport.desktop in ~/.local/share/applications so the launcher and the game show their icon in the dock and the menu. Turning it off removes the entry.",
            ),
            true,
        ),
        check_row(
            state,
            "gamemode",
            restart(lang, "GameMode"),
            tr(
                lang,
                "Asks GameMode (gamemoderun) for the performance governor while the game runs.",
            ) + &missing(tools.gamemode),
            tools.gamemode,
        ),
        check_row(
            state,
            "mangohud",
            restart(lang, "MangoHud overlay"),
            tr(
                lang,
                "Shows MangoHud (FPS, frame times, temperatures) through its Vulkan layer.",
            ) + &missing(tools.mangohud),
            tools.mangohud,
        ),
    ]
}

/// The check boxes of the Diagnostics section.
pub fn diagnostic_checks(state: &State, lang: &str) -> Vec<CheckRow> {
    vec![
        check_row(
            state,
            "frame_stats",
            tr(lang, "Frame statistics in the log (every 5 s)"),
            String::new(),
            true,
        ),
        check_row(
            state,
            "gpu_profile",
            tr(lang, "GPU time per pass in the log"),
            String::new(),
            true,
        ),
        check_row(
            state,
            "perf_diag",
            tr(lang, "GPU wait and readback report in the log (every 5 s)"),
            tr(
                lang,
                "Shows where the frame waits for the GPU and which reads cause it. For performance tests.",
            ),
            true,
        ),
        check_row(
            state,
            "frame_capture_key",
            tr(lang, "F11 captures a frame for analysis"),
            tr(
                lang,
                "Records how the next frame is drawn into the captures folder.",
            ),
            true,
        ),
        check_row(
            state,
            "vk_validation",
            tr(
                lang,
                "Vulkan validation layers (needs the Vulkan SDK; much slower)",
            ),
            String::new(),
            true,
        ),
    ]
}

/// The setting keys of every check box that is shown (Diagnostics only while it is open).
pub fn check_keys(state: &State) -> Vec<&'static str> {
    let lang = state.lang.as_str();
    let mut all = launcher_checks(state, lang);
    all.extend(linux_checks(state, lang));
    if state.diagnostics_open {
        all.extend(diagnostic_checks(state, lang));
    }
    all.into_iter().map(|check| check.key).collect()
}

// --- View ---

fn check_view<'a>(check: &CheckRow) -> Element<'a, Message> {
    let hint = Some(check.hint.as_str());
    if check.enabled {
        return checkbox(check.key, &check.label, check.checked, hint);
    }
    // A tool this system lacks: the box is drawn, but nothing toggles it.
    let boxed = check_box(check.checked)
        .label(check.label.clone())
        .style(theme::checkbox_red);
    let mut body = column![boxed].spacing(2);
    if let Some(hint) = hint.filter(|hint| !hint.is_empty()) {
        body = body.push(text(hint.to_string()).size(13).style(theme::text_muted));
    }
    body.padding([4, 0]).into()
}

fn flat<'a>(label: String, message: Message) -> Element<'a, Message> {
    button(text(label))
        .style(theme::button_flat)
        .on_press(message)
        .into()
}

pub fn view(state: &State) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    let hint = |english: &str| tr(lang, english);
    let selects = rows(state, lang);
    let pick = |key: &str| match selects.iter().find(|row| row.key == key) {
        Some(row) => select_pairs(key, &row.pairs, &row.current),
        None => select_pairs(key, &[], ""),
    };
    let mut page = column![
        section(&tr(lang, "Launcher")),
        row(
            &tr(lang, "Launcher language"),
            pick("ui_language"),
            Some(&hint("Applies when the launcher opens again.")),
        ),
        row(
            &tr(lang, "GUI Scale"),
            pick("ui_scale"),
            Some(&hint("For launcher only. Applies after Save.")),
        ),
    ]
    .spacing(2);
    for check in launcher_checks(state, lang) {
        page = page.push(check_view(&check));
    }

    let mut buttons = layout![].spacing(6).align_y(Center);
    if state.self_update {
        buttons = buttons.push(flat(
            tr(lang, "Check for updates"),
            Message::Update(UpdateMessage::Check { manual: true }),
        ));
    }
    buttons = buttons
        .push(flat(tr(lang, "Desktop shortcut"), Message::DesktopShortcut))
        .push(flat(tr(lang, "Add to Steam"), Message::AddToSteam))
        .push(text(format!("v{VERSION}")).style(theme::text_muted));
    // A narrow drawer in a long language moves the last buttons to a second line.
    page = page.push(column![buttons.wrap().vertical_spacing(6)].padding([10, 0]));

    if !cfg!(windows) {
        page = page.push(section(&tr(lang, "Linux")));
        let linux = linux_checks(state, lang);
        if let Some(first) = linux.first() {
            page = page.push(check_view(first));
        }
        let wayland = if state.linux_tools.wayland {
            String::new()
        } else {
            format!("  {}", tr(lang, "No Wayland session detected."))
        };
        page = page.push(row(
            &restart(lang, "Window system"),
            pick("video_backend"),
            Some(
                &(hint(
                    "Automatic lets SDL choose. Pick X11 if the window misbehaves on Wayland (it runs through XWayland).",
                ) + &wayland),
            ),
        ));
        for check in linux.iter().skip(1) {
            page = page.push(check_view(check));
        }
    }

    let files = layout![
        flat(
            tr(lang, "Port folder"),
            Message::OpenPath(state.paths.data_dir.clone())
        ),
        flat("spartanport.ini".to_string(), Message::OpenIni),
    ]
    .spacing(6);
    let clear = button(text(tr(lang, "Clear shader cache"))).style(theme::button_flat);
    let clear = if clear_cache_blocked(state) {
        clear
    } else {
        clear.on_press(Message::ClearShaderCache)
    };
    page = page.push(section(&tr(lang, "Files"))).push(files).push(
        layout![
            clear,
            text(hint(
                "If the game only shows a black screen, this usually helps."
            ))
            .style(theme::text_muted)
        ]
        .spacing(10)
        .align_y(Center),
    );

    let arrow = if state.diagnostics_open {
        "▾ "
    } else {
        "▸ "
    };
    let title = format!(
        "{arrow}{}",
        tr(lang, "Diagnostics (for development)").to_uppercase()
    );
    page = page.push(
        button(text(title).font(CINZEL_BOLD).size(16))
            .style(theme::button_flat)
            .on_press(Message::ToggleDiagnostics),
    );
    if state.diagnostics_open {
        for check in diagnostic_checks(state, lang) {
            page = page.push(check_view(&check));
        }
    }
    page.into()
}

// --- Actions ---

fn modal(state: &mut State, kind: ModalKind, body: String) {
    state.modal = Some(Modal {
        kind,
        title: APP_NAME.to_string(),
        text: body,
    });
}

/// Removes the shader and pipeline caches; they are rebuilt while playing. A missing folder is
/// fine.
pub fn clear_cache(user_dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(user_dir.join("cache")) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
        _ => Ok(()),
    }
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// The launcher the shortcuts start: the AppImage when there is one, else this executable.
fn launcher_path() -> PathBuf {
    env_var("APPIMAGE")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_default()
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}

/// The desktop shortcut; runs on a thread. Windows makes `SpartanPort.lnk` and returns no files.
fn make_shortcut(paths: &Paths) -> Result<Vec<String>, String> {
    if cfg!(windows) {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        return windows_shortcut(&paths.port_dir, &exe).map(|()| Vec::new());
    }
    let launcher = launcher_path().display().to_string();
    let mut probe = Command::new("xdg-user-dir");
    probe
        .arg("DESKTOP")
        .env_clear()
        .envs(host_environment())
        .stdin(Stdio::null());
    let desktop = probe
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default();
    let desktop = if desktop.is_empty() {
        PathBuf::from(env_var("HOME").unwrap_or_default()).join("Desktop")
    } else {
        PathBuf::from(desktop)
    };
    write_linux_shortcut(
        &format!("\"{launcher}\""),
        &launcher,
        &paths.port_dir.join("launcher").join("spartanport.png"),
        &desktop,
        None,
        &env_var,
    )
    .map_err(|error| error.to_string())
}

/// The arguments after the interpreter that add the game to Steam through
/// `scripts/steam_shortcut.py` (spec 3.5, until the shortcut is ported). `exe` is the program
/// Steam starts; it gets `--play` and starts in the port folder.
pub fn steam_bridge_command(paths: &Paths, settings: &Settings, exe: &Path) -> Vec<String> {
    let port = paths.port_dir.display();
    let script = format!(
        "import sys; sys.path.insert(0, r'{port}/scripts'); import steam_shortcut as s; \
         print(s.add_to_steam(r'{game}', (r'{exe}', '--play', r'{port}'), r'{icon}'))",
        game = settings.get_str("game_dir"),
        exe = exe.display(),
        icon = paths
            .port_dir
            .join("launcher")
            .join("spartanport.png")
            .display(),
    );
    vec!["-c".to_string(), script]
}

/// The number after `'accounts': ` in the printed result.
fn accounts_of(stdout: &str) -> String {
    stdout
        .split("'accounts':")
        .nth(1)
        .map(|rest| {
            rest.trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect()
        })
        .unwrap_or_default()
}

/// What the bridge said, as the text of the dialog: `Ok` for the info, `Err` for the error.
pub fn steam_outcome(
    lang: &str,
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    if success {
        let english = if stdout.contains("'updated': True") {
            "The shortcut \"{}\" was updated in Steam (accounts: {}), with its icon, capsules, background and logo. Open Steam to see it in the library."
        } else {
            "The shortcut \"{}\" was added to Steam (accounts: {}), with its icon, capsules, background and logo. Open Steam to see it in the library."
        };
        return Ok(tr_fmt(lang, english, &[APP_NAME, &accounts_of(stdout)]));
    }
    let known = [
        (
            "SteamError: running",
            "Steam is open. Close it completely (Steam > Exit) and click again: Steam rewrites its shortcuts when it closes.",
        ),
        ("SteamError: no_steam", "Steam was not found for this user."),
        (
            "SteamError: no_art",
            "The game folder has no sce_sys/pic1.png for the pictures.",
        ),
    ];
    if let Some((_, english)) = known.iter().find(|(code, _)| stderr.contains(code)) {
        return Err(tr(lang, english));
    }
    let last = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    Err(last.chars().take(400).collect())
}

/// Runs the bridge; on a thread.
fn add_to_steam(paths: &Paths, settings: &Settings, lang: &str) -> Result<String, String> {
    let game = settings.get_str("game_dir");
    if !Path::new(&game).join("eboot.bin").is_file() {
        return Err(tr(
            lang,
            "Choose the game folder with eboot.bin (CUSA01623).",
        ));
    }
    let python = python_command(&env_var).map_err(|error| error.to_string())?;
    let (program, leading) = python.split_first().ok_or("no Python command")?;
    let mut command = Command::new(program);
    command
        .args(leading)
        .args(steam_bridge_command(paths, settings, &launcher_path()))
        .stdin(Stdio::null());
    no_window(&mut command);
    let output = command.output().map_err(|error| error.to_string())?;
    steam_outcome(
        lang,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Applies the page's events.
pub fn update(state: &mut State, message: Message) -> Task<Message> {
    let lang = state.lang.clone();
    match message {
        Message::ToggleDiagnostics => state.diagnostics_open = !state.diagnostics_open,
        Message::ClearShaderCache => {
            if clear_cache_blocked(state) {
                return Task::none();
            }
            let user = state.paths.user_dir(&state.settings.get_str("user_dir"));
            return blocking(move || clear_cache(&user)).map(Message::ShaderCacheCleared);
        }
        Message::ShaderCacheCleared(Ok(())) => modal(
            state,
            ModalKind::Info,
            tr(
                &lang,
                "Shader cache cleared. The next start stutters for a few minutes while it is rebuilt.",
            ),
        ),
        Message::ShaderCacheCleared(Err(error)) => modal(state, ModalKind::Error, error),
        Message::DesktopShortcut => {
            let paths = state.paths.clone();
            return blocking(move || make_shortcut(&paths)).map(Message::ShortcutDone);
        }
        Message::ShortcutDone(Ok(files)) => {
            let body = if files.is_empty() {
                tr(&lang, "Shortcut created on the desktop.")
            } else {
                format!("{}\n{}", tr(&lang, "Shortcut created:"), files.join("\n"))
            };
            modal(state, ModalKind::Info, body);
        }
        Message::ShortcutDone(Err(error)) => modal(state, ModalKind::Error, error),
        Message::AddToSteam => {
            // The shortcut starts `--play`, which reads the files: they get the window's settings
            // first (Python `add_to_steam`).
            if !persist::collect_or_report(state) {
                return Task::none();
            }
            let (paths, settings) = (state.paths.clone(), state.settings.clone());
            return blocking(move || add_to_steam(&paths, &settings, &lang))
                .map(Message::SteamDone);
        }
        Message::SteamDone(Ok(body)) => modal(state, ModalKind::Info, body),
        Message::SteamDone(Err(error)) => modal(state, ModalKind::Error, error),
        _ => {}
    }
    Task::none()
}
