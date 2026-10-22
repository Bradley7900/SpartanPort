//! The Elm application: state, messages, update, view and subscriptions.

use std::path::PathBuf;
use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key, Modifiers};
use iced::time::Instant;
use iced::widget::{button, column, container, mouse_area, row, space, stack, text};
use iced::{window, Background, Center, Color, Element, Fill, Size, Subscription, Task};
use serde_json::Value;

use crate::core::desktop::open_path;
use crate::core::gpu::{list_displays, monitor_options, monitor_selection, Display, GpuRow};
use crate::core::i18n::{is_rtl, system_language, tr};
use crate::core::ini::{load_ini, Ini, INI_DEFAULTS, INI_FLAGS};
use crate::core::log::{LogBuffer, MAX_LOG_LINES};
use crate::core::options::aspect_of;
use crate::core::paths::Paths;
use crate::core::settings::{load_app_settings, Settings};
use crate::ui::drawer::{self, DrawerState};
use crate::ui::fire::{intensity_for, Fire};
use crate::ui::home::{self, HomeFacts};
use crate::ui::lang_menu::{self, LangMenu, LangMessage};
use crate::ui::pages::controls::ControlsMessage;
use crate::ui::pages::game::{self, BackupMessage, PatchesView};
use crate::ui::pages::mods::{ModsMessage, ModsView};
use crate::ui::pages::performance;
use crate::ui::pages::trophies::{self, TrophiesMessage, TrophiesView};
use crate::ui::pages::Page;
use crate::ui::persist::Persisted;
use crate::ui::play::{GameMessage, GameState};
use crate::ui::splash::{self, SplashMessage, SplashState};
use crate::ui::tasks::blocking;
use crate::ui::theme::{self, palette, CINZEL_BOLD};
use crate::ui::update::{UpdateMessage, UpdateState};
use crate::ui::widgets;
use crate::ui::widgets::modal::{Modal, ModalMessage};
use crate::VERSION;

/// Starting and minimum window size, in logical pixels.
pub const WINDOW_SIZE: Size = Size::new(1280.0, 800.0);
/// Height of the bottom tab bar.
pub const BAR_HEIGHT: f32 = 52.0;
/// Clock period while nothing animates.
const IDLE_CLOCK: Duration = Duration::from_millis(100);
/// Drawer slide duration.
pub const DRAWER_SLIDE: Duration = Duration::from_millis(220);

/// Everything the window shows; only `update` changes it.
pub struct State {
    pub paths: Paths,
    pub settings: Settings,
    pub ini: Ini,
    pub saved: Option<Persisted>,
    pub page: Page,
    pub drawer: DrawerState,
    pub drawer_progress: Animation<bool>,
    pub lang: String,
    /// Whether the interface language reads right to left (Arabic).
    pub rtl: bool,
    pub scale: f32,
    pub splash: SplashState,
    pub game: GameState,
    pub log: LogBuffer,
    pub gpus: Vec<GpuRow>,
    /// The Aspect ratio filter of the Performance page: not saved, it follows the render resolution.
    pub aspect: String,
    /// Sizes the enabled mods add to the render resolutions; see `performance::refresh_mod_resolutions`.
    pub mod_resolutions: Vec<String>,
    pub displays: Vec<Display>,
    pub monitor_pairs: Vec<(String, String)>,
    pub trophies: TrophiesView,
    pub mods: ModsView,
    pub patches: PatchesView,
    /// The names of the save backups, newest first.
    pub backups: Vec<String>,
    /// The backup chosen in the Saves row.
    pub backup_choice: String,
    pub update: UpdateState,
    pub lang_menu: LangMenu,
    /// The art of the Home background, `sce_sys/pic1.png` of the game folder.
    pub home_art: Option<iced::widget::image::Handle>,
    /// What the Home chips read from the disk.
    pub home: HomeFacts,
    pub modal: Option<Modal>,
    pub effects: bool,
    pub fire: Fire,
    /// Whether the window is shown (Task 33 clears it while hidden to the tray).
    pub visible: bool,
    /// Whether the window has the focus.
    pub focused: bool,
    /// Whether the game folder passed the splash check; `None` until it ran.
    pub game_ready: Option<bool>,
    /// Logical window size, for the drawer width.
    pub window_size: Size,
    /// Clock of the animations, advanced by `Tick` and by the messages that start them.
    pub now: Instant,
}

impl State {
    /// Reads `settings.json` and the ini; everything else starts empty.
    pub fn new(paths: Paths) -> State {
        let settings = load_app_settings(&paths);
        let ini = load_ini(&paths.ini_path());
        let lang = match settings.get_str("ui_language") {
            lang if lang.is_empty() => system_language(),
            lang => lang,
        };
        let scale = theme::ui_scale(&settings.get_str("ui_scale"), 1.0);
        let now = Instant::now();
        let rtl = is_rtl(&lang);
        let aspect = aspect_of(
            ini.values
                .get("render_resolution")
                .map_or("", String::as_str),
        )
        .unwrap_or_else(|| "16:9".to_string());
        let mut state = State {
            paths,
            settings,
            ini,
            saved: None,
            page: Page::Home,
            drawer: DrawerState::Closed,
            // EaseOutQuint follows the spec's cubic-bezier(.2, .8, .2, 1) within 0.01; lilt's
            // `Easing::Custom` can take the exact curve if it is ever needed.
            drawer_progress: Animation::new(false)
                .duration(DRAWER_SLIDE)
                .easing(Easing::EaseOutQuint),
            lang,
            rtl,
            scale,
            splash: SplashState::new(now),
            game: Default::default(),
            log: LogBuffer::new(MAX_LOG_LINES),
            gpus: Vec::new(),
            aspect,
            mod_resolutions: Vec::new(),
            displays: Vec::new(),
            monitor_pairs: Vec::new(),
            trophies: Default::default(),
            mods: Default::default(),
            patches: Default::default(),
            backups: Vec::new(),
            backup_choice: String::new(),
            update: Default::default(),
            lang_menu: Default::default(),
            home_art: None,
            home: Default::default(),
            modal: None,
            effects: true,
            fire: Default::default(),
            visible: true,
            focused: true,
            game_ready: None,
            window_size: WINDOW_SIZE,
            now,
        };
        home::load_home_art(&mut state);
        home::refresh_facts(&mut state);
        performance::refresh_mod_resolutions(&mut state);
        game::refresh_game_patches(&mut state);
        game::refresh_backups(&mut state);
        state
    }
}

/// Everything that can happen: clicks, keys, the clock, the window and background work.
#[derive(Debug, Clone)]
pub enum Message {
    ShowPage(Page),
    CloseDrawer,
    NextPage,
    PreviousPage,
    Set(String, Value),
    /// The Aspect ratio box of the Performance page.
    Aspect(String),
    Browse(String),
    Browsed(String, Option<PathBuf>),
    /// A game patch check box of the Game page.
    PatchToggled(String, bool),
    Backup(BackupMessage),
    /// The monitors the probe listed, in the background.
    MonitorsListed(Vec<Display>),
    /// Opens a folder in the desktop's file manager.
    OpenPath(PathBuf),
    Play,
    Stop,
    Save,
    Tick(Instant),
    Key(Key, Modifiers),
    WindowFocused(bool),
    Resized(Size),
    Splash(SplashMessage),
    Lang(LangMessage),
    Game(GameMessage),
    Update(UpdateMessage),
    Modal(ModalMessage),
    Trophies(TrophiesMessage),
    Mods(ModsMessage),
    Controls(ControlsMessage),
    Noop,
}

/// What a key press means here: Esc closes, the arrows change tab, Enter on Home plays.
pub fn key_message(state: &State, key: &Key) -> Option<Message> {
    match key {
        Key::Named(Named::Escape) => Some(Message::CloseDrawer),
        Key::Named(Named::ArrowLeft) => Some(Message::PreviousPage),
        Key::Named(Named::ArrowRight) => Some(Message::NextPage),
        Key::Named(Named::Enter)
            if state.page == Page::Home && state.drawer == DrawerState::Closed =>
        {
            Some(Message::Play)
        }
        _ => None,
    }
}

/// The only place the state changes.
pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::ShowPage(page) => return show_page(state, page),
        // A closed drawer shows Home, so the Home tab is the active one again.
        Message::CloseDrawer => return show_page(state, Page::Home),
        Message::NextPage => return show_page(state, state.page.next()),
        Message::PreviousPage => return show_page(state, state.page.previous()),
        Message::Key(key, _modifiers) => {
            // While the splash shows, any key only skips it.
            if !state.splash.done {
                splash::apply(state, SplashMessage::Skip);
                return Task::none();
            }
            return match key_message(state, &key) {
                Some(Message::Play) => Task::done(Message::Play),
                Some(message) => update(state, message),
                None => Task::none(),
            };
        }
        Message::Resized(size) => state.window_size = size,
        Message::Tick(at) => {
            tick(state, at);
            lang_menu::update(state, LangMessage::Tick(at));
        }
        // Task 33
        Message::Play | Message::Stop => {}
        // Task 32
        Message::Save => {}
        // Task 32 persists what `Set` stored.
        Message::Set(key, value) => apply_set(state, &key, value),
        Message::Aspect(aspect) => performance::choose_aspect(state, &aspect),
        Message::MonitorsListed(displays) => {
            let saved = state.settings.get_str("monitor");
            state.monitor_pairs = monitor_options(&displays);
            state.settings.set(
                "monitor",
                Value::String(monitor_selection(&displays, &saved)),
            );
            state.displays = displays;
        }
        // The result is ignored, as the Python launcher ignores a failed open.
        Message::OpenPath(path) => {
            return blocking(move || {
                let _ = open_path(&path);
            })
            .map(|()| Message::Noop);
        }
        // Task 21 filled Modal(Dismiss)
        Message::Modal(ModalMessage::Dismiss) => state.modal = None,
        Message::Browse(key) => return browse(state, key),
        Message::Browsed(key, Some(path)) => return game::folder_chosen(state, &key, &path),
        Message::Browsed(_, None) => {}
        Message::PatchToggled(name, on) => game::patch_toggled(state, &name, on),
        Message::Backup(message) => return game::backup_update(state, message),
        Message::WindowFocused(focused) => state.focused = focused,
        Message::Splash(message) => splash::apply(state, message),
        Message::Lang(message) => lang_menu::update(state, message),
        // Task 33
        Message::Game(message) => match message {},
        // Task 34
        Message::Update(UpdateMessage::Install) => {}
        Message::Trophies(message) => return trophies::update(state, message),
        // Task 29
        Message::Mods(message) => match message {},
        // Task 28
        Message::Controls(message) => match message {},
        Message::Noop => {}
    }
    Task::none()
}

/// Stores a setting: the ini keys go to the ini values as text, every other key to the app settings.
pub fn apply_set(state: &mut State, key: &str, value: Value) {
    let in_ini = INI_DEFAULTS.iter().any(|(name, _)| *name == key) || INI_FLAGS.contains(&key);
    if !in_ini {
        state.settings.set(key, value);
        return;
    }
    let text = match value {
        Value::Bool(on) => if on { "1" } else { "0" }.to_string(),
        Value::String(text) => text,
        other => other.to_string(),
    };
    state.ini.values.insert(key.to_string(), text);
    if key == "render_resolution" {
        performance::resolution_chosen(state);
    }
}

/// Opens the folder dialog for the setting `key`; the answer comes back as `Browsed`.
fn browse(state: &State, key: String) -> Task<Message> {
    let start = match state.settings.get_str(&key) {
        current if current.is_empty() => state.paths.port_dir.clone(),
        current => PathBuf::from(current),
    };
    let dialog = rfd::AsyncFileDialog::new()
        .set_title(tr(&state.lang, game::browse_prompt(&key)))
        .set_directory(start)
        .pick_folder();
    Task::perform(dialog, move |handle| {
        Message::Browsed(key.clone(), handle.map(|h| h.path().to_path_buf()))
    })
}

/// Lists the monitors in the background; the splash does not wait for it.
pub fn start_monitor_listing(state: &State) -> Task<Message> {
    let paths = state.paths.clone();
    let backend = state.settings.get_str("video_backend");
    blocking(move || list_displays(&paths, &backend)).map(Message::MonitorsListed)
}

/// Advances the clock, the fire and the splash.
fn tick(state: &mut State, at: Instant) {
    let dt = at
        .saturating_duration_since(state.now)
        .as_secs_f32()
        .min(0.1);
    state.now = at;
    state.fire.set_intensity(intensity_for(
        !state.splash.done,
        state.focused,
        state.visible,
        state.effects,
    ));
    // The scene is the window minus the tab bar.
    let scene = Size::new(
        state.window_size.width,
        (state.window_size.height - BAR_HEIGHT).max(0.0),
    );
    state.fire.tick(dt, scene);
    state.splash.advance(at);
}

/// Home closes the drawer; any other page opens it, sliding in when it was closed. The Trophies
/// page reloads its list each time it opens.
fn show_page(state: &mut State, page: Page) -> Task<Message> {
    let now = Instant::now();
    state.now = now;
    state.page = page;
    let open = page != Page::Home;
    state.drawer = if open {
        DrawerState::Open(page)
    } else {
        DrawerState::Closed
    };
    state.drawer_progress.go_mut(open, now);
    if page == Page::Trophies {
        trophies::refresh(state)
    } else {
        Task::none()
    }
}

/// Whether the Home block (PLAY, chips, language, update pill) responds: not behind a wide drawer.
pub fn home_interactive_enabled(state: &State) -> bool {
    !matches!(state.drawer, DrawerState::Open(page) if page.wide())
}

/// Opacity of the Home block: 35 % behind a wide drawer, full otherwise.
pub fn home_interactive_opacity(state: &State) -> f32 {
    if home_interactive_enabled(state) {
        1.0
    } else {
        0.35
    }
}

/// The scene over the tab bar, bottom to top: the Home background, the dim layer that closes
/// the drawer when clicked (only while it is open), the Home block, and the drawer. The Home
/// block sits above the dim layer so it stays clickable next to a 46 % drawer.
pub fn view(state: &State) -> Element<'_, Message> {
    let mut scene = stack![home::background(state)].width(Fill).height(Fill);
    if let DrawerState::Open(_) = state.drawer {
        scene = scene.push(dim_close_catcher());
    }
    scene = scene.push(home::block(state));
    if state.lang_menu.open {
        scene = scene.push(lang_menu::menu(state));
    }
    if !state.splash.done {
        scene = scene.push(splash::view(state, &state.lang));
    }
    if let DrawerState::Open(page) = state.drawer {
        scene = scene.push(drawer::view(state, page));
    }
    let page = column![scene, bar(state)];
    match &state.modal {
        Some(open) => stack![
            page,
            widgets::modal(open.kind.clone(), &open.title, &open.text, &state.lang)
        ]
        .into(),
        None => page.into(),
    }
}

/// Darkens the Home view behind an open drawer; a press on it closes the drawer. The spec also
/// blurs the background, but iced has no cheap backdrop blur, so only the darkening is kept.
fn dim_close_catcher<'a>() -> Element<'a, Message> {
    let dim = container(space())
        .width(Fill)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.45,
                ..palette::BG
            })),
            ..container::Style::default()
        });
    mouse_area(dim).on_press(Message::CloseDrawer).into()
}

/// The bottom bar: the nine tabs and the version.
fn bar(state: &State) -> Element<'_, Message> {
    let tabs = Page::ALL
        .into_iter()
        .fold(row![].spacing(2).height(Fill), |tabs, page| {
            tabs.push(tab(state, page))
        });
    let version = text(format!("v{VERSION}"))
        .size(13)
        .style(theme::text_muted);
    container(
        row![tabs, space::horizontal(), version]
            .align_y(Center)
            .padding([0, 16]),
    )
    .width(Fill)
    .height(BAR_HEIGHT)
    .style(theme::container_panel)
    .into()
}

/// One tab: the label button and, when active, the red underline.
fn tab(state: &State, page: Page) -> Element<'_, Message> {
    let active = state.page == page;
    let label = text(tr(&state.lang, page.label_en()))
        .font(CINZEL_BOLD)
        .size(14)
        .height(Fill)
        .align_y(Center);
    let tab_button = button(label)
        .padding([0, 14])
        .height(Fill)
        .style(theme::tab(active))
        .on_press(Message::ShowPage(page));
    let underline = container(space())
        .width(Fill)
        .height(2)
        .style(move |_| container::Style {
            background: active.then_some(Background::Color(palette::RED_HI)),
            ..container::Style::default()
        });
    column![tab_button, underline].into()
}

/// Keys, window size and, while the drawer slides, animation frames.
pub fn subscription(state: &State) -> Subscription<Message> {
    let keys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => Some(Message::Key(key, modifiers)),
        _ => None,
    });
    let resized = window::resize_events().map(|(_window, size)| Message::Resized(size));
    let frames = if state.drawer_progress.is_animating(state.now) {
        window::frames().map(Message::Tick)
    } else {
        Subscription::none()
    };
    // The splash needs the clock even when the fire is off.
    // The hover timers of the language menu need it too, and a slow clock keeps `now` fresh for
    // their stamps while the fire is off.
    let clock =
        if (state.effects && state.visible) || !state.splash.done || lang_menu::timing(state) {
            iced::time::every(Duration::from_millis(33)).map(Message::Tick)
        } else if state.visible {
            iced::time::every(IDLE_CLOCK).map(Message::Tick)
        } else {
            Subscription::none()
        };
    Subscription::batch([keys, resized, frames, clock])
}

/// Opens the launcher window and runs until it closes.
pub fn run(paths: Paths) -> iced::Result {
    let boot = move || {
        let state = State::new(paths.clone());
        let checks = splash::start_checks(&state);
        let monitors = start_monitor_listing(&state);
        (state, Task::batch([checks, monitors]))
    };
    let app = iced::application(boot, update, view)
        .title("SpartanPort (PC)")
        .theme(|_: &State| theme::theme())
        .subscription(subscription)
        .window(window::Settings {
            size: WINDOW_SIZE,
            min_size: Some(WINDOW_SIZE),
            ..window::Settings::default()
        });
    theme::font_bytes()
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
}
