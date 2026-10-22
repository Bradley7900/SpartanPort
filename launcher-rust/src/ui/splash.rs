//! The splash screen: three parallel start-up checks behind the monogram, then the monogram
//! flies to the top-left corner (spec 4.2), the top-right one in a right-to-left language, where
//! the Home view keeps it.

use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::mouse;
use iced::time::Instant;
use iced::widget::canvas::{self, Geometry};
use iced::widget::{canvas as canvas_widget, container, image, mouse_area, space, stack, text};
use iced::{
    Background, Bottom, Color, Element, Fill, Padding, Point, Rectangle, Renderer, Size, Task,
    Theme,
};

use crate::core::game::ready_to_play;
use crate::core::gpu::{list_gpus, GpuRow};
use crate::core::i18n::tr;
use crate::core::update::{can_self_update, latest_release, Release};
use crate::ui::app::{Message, State, BAR_HEIGHT};
use crate::ui::home;
use crate::ui::tasks::blocking;
use crate::ui::theme::palette;
use crate::ui::update;

/// Splash background (spec 4.2).
pub const SPLASH_BG: Color = Color::from_rgb8(0x07, 0x06, 0x06);
/// The splash never ends before this.
pub const MIN_SHOWN: Duration = Duration::from_millis(800);
/// The splash always ends by this.
pub const MAX_SHOWN: Duration = Duration::from_millis(3000);
/// The shortest time each check's text shows (ruling R86): a third of the minimum, so the three
/// texts take turns even when every check answers at once.
pub const TEXT_TURN: Duration = Duration::from_nanos(MIN_SHOWN.as_nanos() as u64 / 3);
/// The texts of the three checks, in their order.
const CHECK_TEXTS: [&str; 3] = [
    "Checking Vulkan graphics cards…",
    "Checking the game folder…",
    "Checking for updates…",
];
/// Duration of the flight to the corner.
pub const FLY: Duration = Duration::from_millis(700);
/// Monogram size on the splash and in the corner, in logical pixels.
const BIG: f32 = 200.0;
const SMALL: f32 = 60.0;
const CORNER: f32 = 24.0;
/// The red halo behind the monogram (ruling R80, mockup `#slogo` drop shadow
/// `0 0 26px rgba(200, 30, 37, .35)`): the Home glows' soft sprite, its radius a multiple of the
/// monogram's size and its alpha in the middle. Ruling R87: 0.7 × 200 px ends the glow 40 px past
/// the monogram's edge, where 26 px of blur fades out (about 3 standard deviations of 13 px).
const HALO_RADIUS: f32 = 0.7;
const HALO_PEAK: f32 = 0.35;

/// What a finished start-up check found.
#[derive(Debug, Clone)]
pub enum Check {
    Gpus(Vec<GpuRow>),
    GameFolder(bool),
    Update(Option<Release>),
}

impl Check {
    fn index(&self) -> usize {
        match self {
            Check::Gpus(_) => 0,
            Check::GameFolder(_) => 1,
            Check::Update(_) => 2,
        }
    }
}

/// Splash events.
#[derive(Debug, Clone)]
pub enum SplashMessage {
    CheckDone(Check),
    Skip,
    Tick,
}

/// Splash phase: the checks, the clock and the monogram flight.
#[derive(Debug, Clone)]
pub struct SplashState {
    pub started: Instant,
    /// When each check answered: graphics cards, game folder, updates.
    pub done_at: [Option<Instant>; 3],
    pub done: bool,
    pub skipped: bool,
    pub fly: Animation<bool>,
}

impl SplashState {
    pub fn new(now: Instant) -> Self {
        SplashState {
            started: now,
            done_at: [None; 3],
            done: false,
            skipped: false,
            fly: Animation::new(false).duration(FLY).easing(Easing::EaseOut),
        }
    }

    /// When each check's text gives way: once its check answered and the text showed for
    /// `TEXT_TURN`, each turn starting where the one before ended; `None` from the first check that
    /// has not answered.
    fn turn_ends(&self) -> [Option<Instant>; 3] {
        let mut ends = [None; 3];
        let mut start = self.started;
        for (end, done) in ends.iter_mut().zip(self.done_at) {
            let Some(done) = done else { break };
            let finished = (start + TEXT_TURN).max(done);
            *end = Some(finished);
            start = finished;
        }
        ends
    }

    /// Whether the flight should start: the three texts had their turns (so every check
    /// answered) or a skip, after the minimum time; or the maximum time.
    pub fn should_end(&self, now: Instant) -> bool {
        let shown = now.saturating_duration_since(self.started);
        let texts_shown = self.turn_ends()[2].is_some_and(|end| now >= end);
        ((texts_shown || self.skipped) && shown >= MIN_SHOWN) || shown >= MAX_SHOWN
    }

    /// The text whose turn it is at `now` (the last one once all turns ended).
    pub fn current_text(&self, lang: &str, now: Instant) -> String {
        let step = self
            .turn_ends()
            .iter()
            .position(|end| end.is_none_or(|end| now < end))
            .unwrap_or(CHECK_TEXTS.len() - 1);
        tr(lang, CHECK_TEXTS[step])
    }

    /// Whether the flight has begun.
    pub fn flying(&self) -> bool {
        self.fly.value()
    }

    /// Starts the flight when due and ends the splash when it lands.
    pub fn advance(&mut self, now: Instant) {
        if self.done {
            return;
        }
        if !self.flying() {
            if self.should_end(now) {
                self.fly.go_mut(true, now);
            }
        } else if !self.fly.is_animating(now) {
            self.done = true;
        }
    }
}

/// Applies a splash event to the application state.
pub fn apply(state: &mut State, message: SplashMessage) {
    match message {
        SplashMessage::Skip => {
            if !state.splash.done && !state.splash.flying() {
                state.splash.skipped = true;
            }
        }
        SplashMessage::Tick => state.splash.advance(state.now),
        SplashMessage::CheckDone(check) => {
            state.splash.done_at[check.index()] = Some(state.now);
            match check {
                Check::Gpus(gpus) => state.gpus = gpus,
                Check::GameFolder(ready) => state.game_ready = Some(ready),
                Check::Update(Some(release)) => start_up_offer(state, release, cfg!(windows)),
                Check::Update(None) => {}
            }
        }
    }
}

/// The start-up check's release: offered by the same rule as a manual check (newer, and a package
/// for this system or Windows). `windows` is a parameter so tests can cover both systems.
pub fn start_up_offer(state: &mut State, release: Release, windows: bool) {
    if update::offer_decision(&release, false, windows) == update::Decision::Offer {
        update::offer(state, release);
    }
}

/// Starts the three checks in parallel; each answers with `SplashMessage::CheckDone`.
pub fn start_checks(state: &State) -> Task<Message> {
    let done = |check| Message::Splash(SplashMessage::CheckDone(check));
    let paths = state.paths.clone();
    let gpus = blocking(move || list_gpus(&paths)).map(move |rows| done(Check::Gpus(rows)));
    let game_dir = state.settings.get_str("game_dir");
    let update_dir = state.settings.get_str("update_dir");
    let game = blocking(move || ready_to_play(&game_dir, &update_dir))
        .map(move |ready| done(Check::GameFolder(ready)));
    let wants_update = can_self_update(&|key| std::env::var(key).ok(), cfg!(windows))
        && state.settings.get_bool("check_updates");
    let update = if wants_update {
        blocking(|| latest_release(cfg!(windows)).ok())
            .map(move |release| done(Check::Update(release)))
    } else {
        Task::done(done(Check::Update(None)))
    };
    Task::batch([gpus, game, update])
}

/// Where the fire is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashLayers {
    /// The splash draws the fire above its dark background and below the monogram.
    pub fire_on_top_of_background: bool,
    /// The Home background draws the fire (only once the splash is gone), so it is never doubled.
    pub home_draws_fire: bool,
}

pub fn layers(state: &State) -> SplashLayers {
    SplashLayers {
        fire_on_top_of_background: !state.splash.done,
        home_draws_fire: state.splash.done,
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// The halo: the glow sprite around `centre`, `radius` wide, at `opacity` in its middle. A canvas
/// draws it, so it may reach past the window's edges as the monogram flies to the corner.
#[derive(Debug, Clone, Copy)]
struct Halo {
    centre: Point,
    radius: f32,
    opacity: f32,
}

impl canvas::Program<Message> for Halo {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let corner = Point::new(self.centre.x - self.radius, self.centre.y - self.radius);
        frame.draw_image(
            Rectangle::new(corner, Size::new(2.0 * self.radius, 2.0 * self.radius)),
            canvas::Image::new(home::glow_sprite(palette::RED_HI)).opacity(self.opacity),
        );
        vec![frame.into_geometry()]
    }
}

/// The monogram image: the Home corner's handle, made once per process, so the view rebuilt on
/// every tick does not ask wgpu to decode the picture again (it draws nothing while it decodes).
pub fn monogram() -> image::Handle {
    home::monogram()
}

/// The splash layer: background, glowing monogram, status text and hint. During the flight the
/// monogram shrinks to the corner while the background and the texts fade out.
pub fn view<'a>(state: &'a State, lang: &str) -> Element<'a, Message> {
    let splash = &state.splash;
    let t = splash.fly.interpolate(0.0, 1.0, state.now);
    let fade = 1.0 - t;
    let width = state.window_size.width;
    let height = (state.window_size.height - BAR_HEIGHT).max(0.0);

    let size = lerp(BIG, SMALL, t);
    let corner_x = if state.rtl {
        width - CORNER - SMALL
    } else {
        CORNER
    };
    let x = lerp((width - BIG) / 2.0, corner_x, t);
    let y = lerp((height - BIG) / 2.0 - 30.0, CORNER, t);

    let background = container(space())
        .width(Fill)
        .height(Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: fade,
                ..SPLASH_BG
            })),
            ..container::Style::default()
        });

    let halo = canvas_widget(Halo {
        centre: Point::new(x + size / 2.0, y + size / 2.0),
        radius: HALO_RADIUS * size,
        opacity: HALO_PEAK * fade,
    })
    .width(Fill)
    .height(Fill);
    let monogram = image(monogram()).width(size).height(size);
    let monogram_layer = container(monogram)
        .width(Fill)
        .height(Fill)
        .padding(Padding {
            top: y.max(0.0),
            left: x.max(0.0),
            ..Padding::ZERO
        });

    let muted = Color {
        a: fade,
        ..palette::MUTED
    };
    let status = container(
        text(splash.current_text(lang, state.now))
            .size(16)
            .color(muted),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .padding(Padding {
        top: height / 2.0 + BIG / 2.0 + 10.0,
        ..Padding::ZERO
    });
    let hint = container(text(tr(lang, "any key skips")).size(12).color(muted))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .align_y(Bottom)
        .padding(Padding {
            bottom: 18.0,
            ..Padding::ZERO
        });

    let fire = state.fire.view();

    mouse_area(
        stack![background, fire, halo, status, hint, monogram_layer]
            .width(Fill)
            .height(Fill),
    )
    .on_press(Message::Splash(SplashMessage::Skip))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ms(start: Instant, n: u64) -> Instant {
        start + Duration::from_millis(n)
    }

    #[test]
    fn the_fire_is_drawn_inside_the_splash_layer() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| std::process::abort());
        let paths = crate::core::paths::Paths::from_env(dir.path(), &|_| None);
        let mut state = State::new(paths);
        assert!(layers(&state).fire_on_top_of_background);
        assert!(!layers(&state).home_draws_fire);
        state.splash.done = true;
        assert!(!layers(&state).fire_on_top_of_background);
        assert!(layers(&state).home_draws_fire);
    }

    #[test]
    fn every_frame_draws_the_same_monogram_image() {
        // A new handle is a new image to decode: wgpu would draw nothing while it decodes again.
        assert_eq!(monogram().id(), monogram().id());
    }

    #[test]
    fn splash_waits_for_the_three_checks() {
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        assert!(!splash.should_end(ms(start, 1000)));
        splash.done_at = [Some(start), Some(start), None];
        assert!(!splash.should_end(ms(start, 1000)));
        splash.done_at = [Some(start); 3];
        assert!(splash.should_end(ms(start, 1000)));
    }

    #[test]
    fn splash_never_ends_before_800ms() {
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        splash.done_at = [Some(start); 3];
        splash.skipped = true;
        assert!(!splash.should_end(ms(start, 799)));
        assert!(splash.should_end(ms(start, 800)));
    }

    #[test]
    fn splash_ends_at_3000ms_without_checks() {
        let start = Instant::now();
        let splash = SplashState::new(start);
        assert!(!splash.should_end(ms(start, 2999)));
        assert!(splash.should_end(ms(start, 3000)));
    }

    #[test]
    fn skip_ends_after_the_minimum() {
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        splash.skipped = true;
        assert!(!splash.should_end(ms(start, 300)));
        assert!(splash.should_end(ms(start, 900)));
    }

    const GPUS: &str = "Checking Vulkan graphics cards…";
    const GAME: &str = "Checking the game folder…";
    const UPDATES: &str = "Checking for updates…";

    #[test]
    fn the_three_texts_take_turns_when_the_checks_answer_at_once() {
        // Ruling R86: each text for a third of the minimum, though every result is already in.
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        splash.done_at = [Some(start); 3];
        assert_eq!(splash.current_text("en", ms(start, 100)), GPUS);
        assert_eq!(splash.current_text("en", ms(start, 400)), GAME);
        assert_eq!(splash.current_text("en", ms(start, 700)), UPDATES);
        assert!(!splash.should_end(ms(start, 799)));
        assert!(splash.should_end(ms(start, 800)));
    }

    #[test]
    fn a_text_stays_until_its_check_answers() {
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        // Nothing answered: the first text, however long it takes.
        assert_eq!(splash.current_text("en", ms(start, 2000)), GPUS);
        // The graphics cards answer at 1 s: the game folder and the updates take their turns after.
        splash.done_at = [Some(ms(start, 1000)), Some(start), Some(start)];
        assert_eq!(splash.current_text("en", ms(start, 999)), GPUS);
        assert_eq!(splash.current_text("en", ms(start, 1100)), GAME);
        assert_eq!(splash.current_text("en", ms(start, 1300)), UPDATES);
        assert!(!splash.should_end(ms(start, 1500)));
        assert!(splash.should_end(ms(start, 1534)));
        // A check still running holds its text, until the 3 s limit.
        splash.done_at = [Some(start), None, Some(start)];
        assert_eq!(splash.current_text("en", ms(start, 2000)), GAME);
        assert!(!splash.should_end(ms(start, 2999)));
        assert!(splash.should_end(ms(start, 3000)));
    }
}
