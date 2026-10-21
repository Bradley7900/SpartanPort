//! The Elm application: state, messages, update, view and subscriptions.

use std::path::PathBuf;
use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key, Modifiers};
use iced::time::Instant;
use iced::widget::{button, column, container, mouse_area, row, space, stack, text};
use iced::{
    window, Background, Bottom, Center, Color, Element, Fill, Right, Size, Subscription, Task,
};
use serde_json::Value;

use crate::core::gpu::{Display, GpuRow};
use crate::core::i18n::{system_language, tr};
use crate::core::ini::{load_ini, Ini};
use crate::core::log::{LogBuffer, MAX_LOG_LINES};
use crate::core::paths::Paths;
use crate::core::settings::{load_app_settings, Settings};
use crate::ui::drawer::{self, DrawerState};
use crate::ui::fire::Fire;
use crate::ui::lang_menu::{LangMenu, LangMessage};
use crate::ui::pages::controls::ControlsMessage;
use crate::ui::pages::game::PatchesView;
use crate::ui::pages::mods::{ModsMessage, ModsView};
use crate::ui::pages::trophies::{TrophiesMessage, TrophiesView};
use crate::ui::pages::Page;
use crate::ui::persist::Persisted;
use crate::ui::play::{GameMessage, GameState};
use crate::ui::splash::{SplashMessage, SplashState};
use crate::ui::theme::{self, palette, CINZEL_BOLD};
use crate::ui::update::{UpdateMessage, UpdateState};
use crate::ui::widgets;
use crate::ui::widgets::modal::{Modal, ModalMessage};
use crate::VERSION;

/// Starting and minimum window size, in logical pixels.
pub const WINDOW_SIZE: Size = Size::new(1280.0, 800.0);
/// Height of the bottom tab bar.
pub const BAR_HEIGHT: f32 = 52.0;
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
    pub scale: f32,
    pub splash: SplashState,
    pub game: GameState,
    pub log: LogBuffer,
    pub gpus: Vec<GpuRow>,
    pub displays: Vec<Display>,
    pub monitor_pairs: Vec<(String, String)>,
    pub trophies: TrophiesView,
    pub mods: ModsView,
    pub patches: PatchesView,
    pub update: UpdateState,
    pub lang_menu: LangMenu,
    pub modal: Option<Modal>,
    pub effects: bool,
    pub fire: Fire,
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
        State {
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
            scale,
            splash: Default::default(),
            game: Default::default(),
            log: LogBuffer::new(MAX_LOG_LINES),
            gpus: Vec::new(),
            displays: Vec::new(),
            monitor_pairs: Vec::new(),
            trophies: Default::default(),
            mods: Default::default(),
            patches: Default::default(),
            update: Default::default(),
            lang_menu: Default::default(),
            modal: None,
            effects: false,
            fire: Default::default(),
            window_size: WINDOW_SIZE,
            now: Instant::now(),
        }
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
    Browse(String),
    Browsed(String, Option<PathBuf>),
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
        Message::ShowPage(page) => show_page(state, page),
        // A closed drawer shows Home, so the Home tab is the active one again.
        Message::CloseDrawer => show_page(state, Page::Home),
        Message::NextPage => show_page(state, state.page.next()),
        Message::PreviousPage => show_page(state, state.page.previous()),
        Message::Key(key, _modifiers) => {
            return match key_message(state, &key) {
                Some(Message::Play) => Task::done(Message::Play),
                Some(message) => update(state, message),
                None => Task::none(),
            };
        }
        Message::Resized(size) => state.window_size = size,
        // Task 22 also advances the splash and the fire here.
        Message::Tick(at) => state.now = at,
        // Task 33
        Message::Play | Message::Stop => {}
        // Task 32
        Message::Save => {}
        // Task 32 (save flow) handles Set
        Message::Set(_, _) => {}
        // Task 21 filled Modal(Dismiss)
        Message::Modal(ModalMessage::Dismiss) => state.modal = None,
        // Task 26
        Message::Browse(_) | Message::Browsed(_, _) => {}
        // Task 22
        Message::WindowFocused(_) => {}
        Message::Splash(message) => match message {},
        // Task 23
        Message::Lang(message) => match message {},
        // Task 33
        Message::Game(message) => match message {},
        // Task 34
        Message::Update(message) => match message {},
        // Task 27
        Message::Trophies(message) => match message {},
        // Task 29
        Message::Mods(message) => match message {},
        // Task 28
        Message::Controls(message) => match message {},
        Message::Noop => {}
    }
    Task::none()
}

/// Home closes the drawer; any other page opens it, sliding in when it was closed.
fn show_page(state: &mut State, page: Page) {
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
    let home_background = container(space())
        .width(Fill)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(palette::BG)),
            ..container::Style::default()
        });
    let mut scene = stack![home_background].width(Fill).height(Fill);
    if let DrawerState::Open(_) = state.drawer {
        scene = scene.push(dim_close_catcher());
    }
    scene = scene.push(home_interactive(state));
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

/// The right-hand Home block, anchored bottom-right; Task 23 fills it and applies
/// `home_interactive_opacity` and `home_interactive_enabled`.
fn home_interactive(_state: &State) -> Element<'_, Message> {
    // Task 23 fills this
    container(space())
        .width(Fill)
        .height(Fill)
        .align_x(Right)
        .align_y(Bottom)
        .into()
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
    Subscription::batch([keys, resized, frames])
}

/// Opens the launcher window and runs until it closes.
pub fn run(paths: Paths) -> iced::Result {
    let app = iced::application(move || State::new(paths.clone()), update, view)
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
