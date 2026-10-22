//! The splash screen: three parallel start-up checks behind the monogram, then the monogram
//! flies to the top-left corner (spec 4.2), the top-right one in a right-to-left language, where
//! the Home view keeps it.

use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::time::Instant;
use iced::widget::{container, image, mouse_area, space, stack, text};
use iced::{Background, Border, Bottom, Color, Element, Fill, Padding, Task};

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
/// Duration of the flight to the corner.
pub const FLY: Duration = Duration::from_millis(700);
/// Monogram size on the splash and in the corner, in logical pixels.
const BIG: f32 = 200.0;
const SMALL: f32 = 60.0;
const CORNER: f32 = 24.0;
/// Width of the red halo around the monogram.
const GLOW: f32 = 80.0;

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
    pub checks: [bool; 3],
    pub done: bool,
    pub skipped: bool,
    pub fly: Animation<bool>,
}

impl SplashState {
    pub fn new(now: Instant) -> Self {
        SplashState {
            started: now,
            checks: [false; 3],
            done: false,
            skipped: false,
            fly: Animation::new(false).duration(FLY).easing(Easing::EaseOut),
        }
    }

    /// Whether the flight should start: every check or a skip, after the minimum time; or the
    /// maximum time.
    pub fn should_end(&self, now: Instant) -> bool {
        let shown = now.saturating_duration_since(self.started);
        ((self.checks.iter().all(|done| *done) || self.skipped) && shown >= MIN_SHOWN)
            || shown >= MAX_SHOWN
    }

    /// The text of the first unfinished check (the last one when all are done).
    pub fn current_text(&self, lang: &str) -> String {
        let texts = [
            "Checking Vulkan graphics cards…",
            "Checking the game folder…",
            "Checking for updates…",
        ];
        let step = self.checks.iter().position(|done| !done).unwrap_or(2);
        tr(lang, texts[step])
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
            state.splash.checks[check.index()] = true;
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
    let glow = lerp(GLOW, 0.0, t);

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

    let monogram = image(monogram()).width(size).height(size);
    let halo = container(monogram)
        .padding(glow)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.16 * fade,
                ..palette::RED
            })),
            border: Border {
                radius: (size + 2.0 * glow).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let monogram_layer = container(halo).width(Fill).height(Fill).padding(Padding {
        top: (y - glow).max(0.0),
        left: (x - glow).max(0.0),
        ..Padding::ZERO
    });

    let muted = Color {
        a: fade,
        ..palette::MUTED
    };
    let status = container(text(splash.current_text(lang)).size(16).color(muted))
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
        stack![background, fire, status, hint, monogram_layer]
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
        splash.checks = [true, true, false];
        assert!(!splash.should_end(ms(start, 1000)));
        splash.checks = [true; 3];
        assert!(splash.should_end(ms(start, 1000)));
    }

    #[test]
    fn splash_never_ends_before_800ms() {
        let start = Instant::now();
        let mut splash = SplashState::new(start);
        splash.checks = [true; 3];
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

    #[test]
    fn status_text_follows_the_checks() {
        let mut splash = SplashState::new(Instant::now());
        assert_eq!(splash.current_text("en"), "Checking Vulkan graphics cards…");
        splash.checks = [true, false, false];
        assert_eq!(splash.current_text("en"), "Checking the game folder…");
        splash.checks = [true, true, false];
        assert_eq!(splash.current_text("en"), "Checking for updates…");
        splash.checks = [true; 3];
        assert_eq!(splash.current_text("en"), "Checking for updates…");
    }
}
