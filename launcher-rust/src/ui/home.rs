//! The Home page: layered background (spec 4.3) and the interactive block over it.

use std::f32::consts::TAU;
use std::path::Path;
use std::sync::OnceLock;

use iced::mouse;
use iced::widget::canvas::{self, gradient, Geometry, Path as Shape};
use iced::widget::image::Handle;
use iced::widget::{
    button, canvas as canvas_widget, column, container, image, row, space, stack, text,
};
use iced::{
    Background, Bottom, Center, Color, ContentFit, Element, Fill, Left, Padding, Point, Radians,
    Rectangle, Renderer, Right, Size, Theme, Top, Vector,
};

use crate::core::game::{game_info, game_patchable, game_profile_of, meta_folder, GameInfo};
use crate::core::gpu::chosen_gpu;
use crate::core::i18n::{tr, tr_fmt};
use crate::core::options::{Choice, DISPLAY_MODES, FPS_LIMITS, OUTPUTS, PRESETS, UPSCALERS};
use crate::core::patches::patch_status;
use crate::ui::app::{
    home_interactive_enabled, home_interactive_opacity, scene_size, Message, State,
};
use crate::ui::lang_menu;
use crate::ui::pages::performance;
use crate::ui::play::GameState;
use crate::ui::splash;
use crate::ui::theme::{
    button_flat, button_play, mirror_padding, mirror_x, palette, text_muted, CINZEL_BLACK,
};
use crate::ui::update::{UpdateMessage, UpdateState};
use crate::ui::widgets::{chip, ChipKind};

// --- Background layers (spec 4.3, mockup `launcher-mockup-v13.html`) ---

/// Layer 1: the base gradient, top to bottom.
const BASE_TOP: Color = Color::from_rgb(
    0x1b as f32 / 255.0,
    0x14 as f32 / 255.0,
    0x14 as f32 / 255.0,
);
/// Red glow at (68 %, 38 %): colour, peak alpha, radii as a share of width and height.
const RED_GLOW: (f32, f32, f32) = (200.0 / 255.0, 30.0 / 255.0, 37.0 / 255.0);
const RED_GLOW_AT: (f32, f32) = (0.68, 0.38);
const RED_GLOW_PEAK: f32 = 0.42;
const RED_GLOW_RADII: (f32, f32) = (0.38, 0.46);
/// Grey glow at (22 %, 78 %).
const GREY_GLOW: (f32, f32, f32) = (130.0 / 255.0, 130.0 / 255.0, 130.0 / 255.0);
const GREY_GLOW_AT: (f32, f32) = (0.22, 0.78);
const GREY_GLOW_PEAK: f32 = 0.14;
const GREY_GLOW_RADII: (f32, f32) = (0.25, 0.32);
/// Concentric discs that fake a radial falloff (the canvas has no radial gradient).
const GLOW_STEPS: u32 = 10;
/// Layer 2: the launcher's key art (`assets/background.png`, mockup `#photo`): its size in pixels,
/// its opacity, and the point `cover` keeps in place (CSS `background-position: 70% 30%`).
const ART_SIZE: (u32, u32) = (1920, 1080);
const ART_OPACITY: f32 = 0.7;
const ART_ANCHOR: (f32, f32) = (0.70, 0.30);
/// The id of the container that holds the key art.
pub const ART_ID: &str = "home-art";
/// Layer 3: uniform dark veil, left-hand darkening and its reach, red rising from the floor.
const VEIL_ALPHA: f32 = 0.32;
const LEFT_VEIL_REACH: f32 = 0.58;
const LEFT_VEIL_STOPS: [(f32, f32); 3] = [(0.0, 0.88), (0.30, 0.45), (0.58, 0.0)];
const BOTTOM_RED: (f32, f32, f32) = (140.0 / 255.0, 16.0 / 255.0, 20.0 / 255.0);
/// Layer 4: the fog quad is 140 % of the width and 70 % of the height, its bottom edge 6 % below
/// the view, and it is turned by -7 degrees about the middle of that edge.
const FOG_WIDTH: f32 = 1.4;
const FOG_HEIGHT: f32 = 0.70;
const FOG_BELOW: f32 = 0.06;
const FOG_ANGLE_DEGREES: f32 = -7.0;
const FOG_RED: (f32, f32, f32, f32) = (110.0 / 255.0, 12.0 / 255.0, 16.0 / 255.0, 0.38);
const FOG_FLOOR_ALPHA: f32 = 0.96;
/// Layer 5: the orange heat pulses once per period between these alphas.
const HEAT_PERIOD: f32 = 2.8;
const HEAT_MIN: f32 = 0.65;
const HEAT_MAX: f32 = 1.0;
/// One heat blob: centre and radii (shares of width and height), colour and peak alpha.
struct HeatBlob {
    centre: (f32, f32),
    radii: (f32, f32),
    colour: (f32, f32, f32),
    peak: f32,
}

/// The three heat blobs along the floor (mockup `#heat`).
const HEAT_BLOBS: [HeatBlob; 3] = [
    HeatBlob {
        centre: (0.52, 1.04),
        radii: (0.45, 0.30),
        colour: (235.0 / 255.0, 80.0 / 255.0, 30.0 / 255.0),
        peak: 0.55,
    },
    HeatBlob {
        centre: (0.18, 1.10),
        radii: (0.20, 0.20),
        colour: (230.0 / 255.0, 90.0 / 255.0, 30.0 / 255.0),
        peak: 0.30,
    },
    HeatBlob {
        centre: (0.84, 0.96),
        radii: (0.30, 0.30),
        colour: (235.0 / 255.0, 90.0 / 255.0, 30.0 / 255.0),
        peak: 0.50,
    },
];

// --- Home block (mockup `#home`, `#topright`, `.chips`, `#quick`) ---

/// Logotype height in the block.
const LOGO_HEIGHT: f32 = 54.0;
/// Corner monogram size and its distance from the edges (the splash flies it to the same place).
const MONOGRAM: f32 = 60.0;
const MONOGRAM_EDGE: f32 = 24.0;
/// PLAY: font size and padding.
const PLAY_SIZE: f32 = 22.0;
const PLAY_PADDING: [f32; 2] = [16.0, 54.0];
/// Mockup `#home`: 40 px from the right; 78 px from the window bottom, less the 52 px tab bar.
const BLOCK_RIGHT: f32 = 40.0;
const BLOCK_BOTTOM: f32 = 26.0;
/// Mockup `#topright`.
const TOP_RIGHT_TOP: f32 = 22.0;
const TOP_RIGHT_RIGHT: f32 = 24.0;

/// What the Home chips need from the disk, read once so `view` does no I/O.
#[derive(Debug, Clone, Default)]
pub struct HomeFacts {
    pub info: Option<GameInfo>,
    pub title_id: Option<String>,
    pub patchable: bool,
    pub saves: bool,
    /// Patches the last start left out (`patch_status.json`).
    pub skipped: usize,
}

/// Re-reads the game folder, the saves folder and the patch report for the chips.
pub fn refresh_facts(state: &mut State) {
    let meta = meta_folder(
        &state.settings.get_str("game_dir"),
        &state.settings.get_str("update_dir"),
    );
    let (title_id, _profile) = game_profile_of(&meta);
    let savedata = user_dir(state).join("savedata");
    state.home = HomeFacts {
        info: game_info(&meta),
        title_id,
        patchable: game_patchable(&meta),
        saves: has_files(&savedata),
        skipped: patch_status(&state.paths.data_dir.join("out"))
            .map_or(0, |status| status.skipped.len()),
    };
}

fn user_dir(state: &State) -> std::path::PathBuf {
    state.paths.user_dir(&state.settings.get_str("user_dir"))
}

/// Whether `folder` holds a file at any depth (`save_backup._has_saves`).
fn has_files(folder: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            has_files(&path)
        } else {
            path.is_file()
        }
    })
}

// --- Canvas layers ---

#[derive(Debug, Clone, Copy)]
enum Layer {
    Base,
    Veil,
    Fog,
    /// Seconds since the launcher started.
    Heat(f32),
}

#[derive(Debug, Clone, Copy)]
struct Backdrop(Layer);

fn rgba((r, g, b): (f32, f32, f32), a: f32) -> Color {
    Color::from_rgba(r, g, b, a)
}

fn bg(alpha: f32) -> Color {
    Color {
        a: alpha,
        ..palette::BG
    }
}

/// An elliptical glow made of concentric discs whose alphas add up to `peak` at the centre.
fn glow(
    frame: &mut canvas::Frame,
    centre: Point,
    radii: Vector,
    colour: (f32, f32, f32),
    peak: f32,
) {
    let step_alpha = 1.0 - (1.0 - peak.clamp(0.0, 0.99)).powf(1.0 / GLOW_STEPS as f32);
    frame.with_save(|frame| {
        frame.translate(Vector::new(centre.x, centre.y));
        frame.scale_nonuniform(radii);
        for step in 0..GLOW_STEPS {
            let radius = 1.0 - step as f32 / GLOW_STEPS as f32;
            frame.fill(
                &Shape::circle(Point::ORIGIN, radius),
                rgba(colour, step_alpha),
            );
        }
    });
}

fn vertical(size: Size, stops: &[(f32, Color)]) -> gradient::Linear {
    stops.iter().fold(
        gradient::Linear::new(Point::ORIGIN, Point::new(0.0, size.height)),
        |line, (at, colour)| line.add_stop(*at, *colour),
    )
}

/// `point` turned by `angle` about `pivot` (y points down, so a negative angle turns it
/// counter-clockwise on screen, like CSS `rotate(-7deg)`).
fn turned(point: Vector, pivot: Point, angle: Radians) -> Point {
    let (sin, cos) = angle.0.sin_cos();
    Point::new(
        pivot.x + point.x * cos - point.y * sin,
        pivot.y + point.x * sin + point.y * cos,
    )
}

/// The red and black fog quad of layer 4.
fn paint_fog(frame: &mut canvas::Frame, size: Size) {
    let pivot = Point::new(size.width / 2.0, size.height * (1.0 + FOG_BELOW));
    let angle = Radians(FOG_ANGLE_DEGREES.to_radians());
    let half = FOG_WIDTH * size.width / 2.0;
    let tall = FOG_HEIGHT * size.height;
    let corners = [(-half, -tall), (half, -tall), (half, 0.0), (-half, 0.0)]
        .map(|(x, y)| turned(Vector::new(x, y), pivot, angle));
    let quad = Shape::new(|path| {
        path.move_to(corners[0]);
        path.line_to(corners[1]);
        path.line_to(corners[2]);
        path.line_to(corners[3]);
        path.close();
    });
    let (r, g, b, a) = FOG_RED;
    let line = gradient::Linear::new(
        turned(Vector::new(0.0, -tall), pivot, angle),
        turned(Vector::new(0.0, 0.0), pivot, angle),
    )
    .add_stop(0.0, Color::from_rgba(r, g, b, 0.0))
    .add_stop(0.5, Color::from_rgba(r, g, b, a))
    .add_stop(1.0, bg(FOG_FLOOR_ALPHA));
    frame.fill(&quad, line);
}

/// Heat alpha multiplier at `elapsed` seconds: a sine between `HEAT_MIN` and `HEAT_MAX`.
pub fn heat_pulse(elapsed: f32) -> f32 {
    let wave = 0.5 + 0.5 * (elapsed / HEAT_PERIOD * TAU).sin();
    HEAT_MIN + (HEAT_MAX - HEAT_MIN) * wave
}

impl canvas::Program<Message> for Backdrop {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = bounds.size();
        let mut frame = canvas::Frame::new(renderer, size);
        let at = |(x, y): (f32, f32)| Point::new(x * size.width, y * size.height);
        let radii = |(x, y): (f32, f32)| Vector::new(x * size.width, y * size.height);
        match self.0 {
            Layer::Base => {
                frame.fill_rectangle(
                    Point::ORIGIN,
                    size,
                    vertical(size, &[(0.0, BASE_TOP), (1.0, palette::BG)]),
                );
                glow(
                    &mut frame,
                    at(RED_GLOW_AT),
                    radii(RED_GLOW_RADII),
                    RED_GLOW,
                    RED_GLOW_PEAK,
                );
                glow(
                    &mut frame,
                    at(GREY_GLOW_AT),
                    radii(GREY_GLOW_RADII),
                    GREY_GLOW,
                    GREY_GLOW_PEAK,
                );
            }
            Layer::Veil => {
                frame.fill_rectangle(Point::ORIGIN, size, bg(VEIL_ALPHA));
                let reach = LEFT_VEIL_REACH * size.width;
                let left = LEFT_VEIL_STOPS.iter().fold(
                    gradient::Linear::new(Point::ORIGIN, Point::new(reach, 0.0)),
                    |line, (share, alpha)| line.add_stop(share / LEFT_VEIL_REACH, bg(*alpha)),
                );
                frame.fill_rectangle(Point::ORIGIN, Size::new(reach, size.height), left);
                let red = |a| rgba(BOTTOM_RED, a);
                frame.fill_rectangle(
                    Point::ORIGIN,
                    size,
                    vertical(
                        size,
                        &[
                            (0.0, bg(0.6)),
                            (0.28, bg(0.0)),
                            (0.5, red(0.0)),
                            (0.78, red(0.5)),
                            (1.0, bg(0.98)),
                        ],
                    ),
                );
            }
            Layer::Fog => paint_fog(&mut frame, size),
            Layer::Heat(elapsed) => {
                let pulse = heat_pulse(elapsed);
                for blob in HEAT_BLOBS {
                    glow(
                        &mut frame,
                        at(blob.centre),
                        radii(blob.radii),
                        blob.colour,
                        blob.peak * pulse,
                    );
                }
            }
        }
        vec![frame.into_geometry()]
    }
}

fn layer<'a>(which: Layer) -> Element<'a, Message> {
    canvas_widget(Backdrop(which))
        .width(Fill)
        .height(Fill)
        .into()
}

/// The launcher's key art, one handle per process.
pub fn art() -> Handle {
    static ART: OnceLock<Handle> = OnceLock::new();
    ART.get_or_init(|| Handle::from_bytes(&include_bytes!("../../assets/background.png")[..]))
        .clone()
}

/// The part of the key art that a `cover` fit shows in `scene`: the picture scaled to cover the
/// scene, cut to its shape around `ART_ANCHOR`. An empty scene shows the whole picture.
pub fn art_crop(scene: Size) -> Rectangle<u32> {
    let full = Size::new(ART_SIZE.0 as f32, ART_SIZE.1 as f32);
    let scale = (scene.width / full.width).max(scene.height / full.height);
    if !scale.is_finite() || scale <= 0.0 {
        return Rectangle {
            x: 0,
            y: 0,
            width: ART_SIZE.0,
            height: ART_SIZE.1,
        };
    }
    let shown = Size::new(
        (scene.width / scale).min(full.width),
        (scene.height / scale).min(full.height),
    );
    Rectangle {
        x: ((full.width - shown.width) * ART_ANCHOR.0).round() as u32,
        y: ((full.height - shown.height) * ART_ANCHOR.1).round() as u32,
        width: shown.width.round() as u32,
        height: shown.height.round() as u32,
    }
}

/// The five layers of spec 4.3, bottom to top, with the fire drawn over them.
pub fn background(state: &State) -> Element<'_, Message> {
    let elapsed = state
        .now
        .saturating_duration_since(state.splash.started)
        .as_secs_f32();
    let art = image(art())
        .width(Fill)
        .height(Fill)
        .content_fit(ContentFit::Cover)
        .crop(art_crop(scene_size(state)))
        .opacity(ART_OPACITY);
    let mut layers = stack![
        layer(Layer::Base),
        container(art).width(Fill).height(Fill).id(ART_ID)
    ]
    .width(Fill)
    .height(Fill)
    .push(layer(Layer::Veil))
    .push(layer(Layer::Fog))
    .push(layer(Layer::Heat(elapsed)));
    // While the splash is live it draws the fire itself, above its own dark background.
    if splash::layers(state).home_draws_fire {
        layers = layers.push(state.fire.view());
    }
    layers.into()
}

// --- Texts ---

fn choice_label(table: &[Choice], value: &str, lang: &str) -> String {
    table
        .iter()
        .find(|choice| choice.value == value)
        .map_or_else(|| value.to_string(), |choice| tr(lang, choice.en))
}

fn ini_value<'a>(state: &'a State, key: &str) -> &'a str {
    state.ini.values.get(key).map_or("", String::as_str)
}

/// The patches line (`summary()` of the 2.6.0 launcher).
pub fn summary(state: &State) -> String {
    let lang = state.lang.as_str();
    // The label of the render resolution list (with its ratio and the `(mod)` mark), as the Python.
    let current = ini_value(state, "render_resolution");
    let resolution = performance::resolution_options(state)
        .into_iter()
        .find(|(value, _)| value == current)
        .map_or_else(|| "?".to_string(), |(_, label)| label);
    if !state.home.patchable {
        return tr_fmt(
            lang,
            "{} · 60 FPS engine · no patches for this game",
            &[&resolution],
        );
    }
    let count = state
        .home
        .title_id
        .as_deref()
        .and_then(|id| state.settings.get("game_patches")?.get(id)?.as_array())
        .map_or(0, Vec::len);
    let mut line = tr_fmt(
        lang,
        "{} · {} FPS engine · {} patches",
        &[
            &resolution,
            ini_value(state, "engine_fps"),
            &count.to_string(),
        ],
    );
    if state.home.skipped > 0 {
        line.push_str(" · ");
        line.push_str(&tr_fmt(
            lang,
            "{} not found in this eboot.bin at the last start (see Log)",
            &[&state.home.skipped.to_string()],
        ));
    }
    line
}

/// The four chips: graphics card, game, saves, and the status (the patches summary at rest).
pub fn chips(state: &State) -> [String; 4] {
    let lang = state.lang.as_str();
    let gpu = match chosen_gpu(&state.gpus, &state.settings.get_str("gpu_id")) {
        // The text of the 2.6.0 tables; the probe lists the cards without their Vulkan version.
        Some(row) => tr_fmt(lang, "✓ Graphics card: {}", &[&row.name]),
        None => tr(lang, "• Graphics card: not checked"),
    };
    let game = match &state.home.info {
        Some(info) => {
            let id = state.home.title_id.as_deref().unwrap_or(&info.title);
            format!("✓ {id} · v{}", info.version)
        }
        None => tr(lang, "✗ No eboot.bin in the game folder"),
    };
    let saves = if state.home.saves {
        let dir = user_dir(state).display().to_string();
        tr_fmt(lang, "✓ Saves found in {}", &[&dir])
    } else {
        tr(lang, "• No saves yet")
    };
    [gpu, game, saves, status_text(state)]
}

/// The status chip (Python `self.status`): what the game is doing while it starts, runs or its
/// system files are copied; the patches summary otherwise.
pub fn status_text(state: &State) -> String {
    let busy = !matches!(state.game, GameState::Idle);
    if busy && !state.status.is_empty() {
        state.status.clone()
    } else if state.home.info.is_some() {
        summary(state)
    } else {
        format!("• {}", tr(&state.lang, "Choose the game folder first."))
    }
}

/// The settings line: upscaler, preset, output, FPS limit, window mode and monitor.
pub fn quick_line(state: &State) -> String {
    let lang = state.lang.as_str();
    let fps = match ini_value(state, "fps_limit") {
        "0" => choice_label(FPS_LIMITS, "0", lang),
        limit => format!("{limit} FPS"),
    };
    let monitor = match state.settings.get_str("monitor") {
        monitor if monitor.is_empty() => tr(lang, "Primary"),
        monitor => monitor,
    };
    [
        choice_label(UPSCALERS, ini_value(state, "upscaler"), lang),
        choice_label(PRESETS, ini_value(state, "preset"), lang),
        choice_label(OUTPUTS, ini_value(state, "output_res"), lang),
        fps,
        choice_label(DISPLAY_MODES, ini_value(state, "display_mode"), lang),
        format!("{} {monitor}", tr(lang, "Monitor")),
    ]
    .join(" · ")
}

/// The chip colour follows the mark the text starts with.
fn chip_kind(label: &str) -> ChipKind {
    if label.starts_with('✓') {
        ChipKind::Ok
    } else if label.starts_with('✗') {
        ChipKind::Warn
    } else {
        ChipKind::Plain
    }
}

/// The text of the pill: "Version X is available · Update".
pub fn update_pill_text(lang: &str, version: &str) -> String {
    let available = tr_fmt(lang, "Version {} is available.", &[version]);
    format!(
        "{} · {}",
        available.trim_end_matches('.'),
        tr(lang, "Update")
    )
}

// --- The block ---

fn logo() -> Handle {
    static LOGO: OnceLock<Handle> = OnceLock::new();
    LOGO.get_or_init(|| {
        Handle::from_bytes(&include_bytes!("../../assets/spartanport-logo.png")[..])
    })
    .clone()
}

/// The monogram image, one handle per process (the splash shows the same one).
pub fn monogram() -> Handle {
    static MONOGRAM_IMAGE: OnceLock<Handle> = OnceLock::new();
    MONOGRAM_IMAGE
        .get_or_init(|| Handle::from_bytes(&include_bytes!("../../assets/spartanport.png")[..]))
        .clone()
}

/// Fades `content` to the Home opacity (35 % behind a wide drawer) by darkening it.
fn dimmed<'a>(content: Element<'a, Message>, state: &State) -> Element<'a, Message> {
    let opacity = home_interactive_opacity(state);
    if opacity >= 1.0 {
        return content;
    }
    let shade = container(space())
        .width(Fill)
        .height(Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg(1.0 - opacity))),
            ..container::Style::default()
        });
    stack![content, shade].into()
}

fn play_row(state: &State, enabled: bool) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    // PLAY waits while the system files are copied; a started game turns it into RUNNING and STOP.
    let (running, copying) = match state.game {
        GameState::Idle => (false, false),
        GameState::Copying => (false, true),
        GameState::Preparing { .. } | GameState::Running { .. } => (true, false),
    };
    let (label, press) = if running {
        (tr(lang, "RUNNING"), None)
    } else {
        (
            tr(lang, "PLAY"),
            (enabled && !copying).then_some(Message::Play),
        )
    };
    let play = button(text(label).font(CINZEL_BLACK).size(PLAY_SIZE))
        .padding(PLAY_PADDING)
        .style(button_play)
        .on_press_maybe(press);
    let mut line = row![].spacing(12).align_y(Center);
    if running {
        line = line.push(
            button(text(tr(lang, "Stop").to_uppercase()).size(14))
                .padding([14, 22])
                .style(button_flat)
                .on_press_maybe(enabled.then_some(Message::Stop)),
        );
    }
    line.push(play).into()
}

fn bottom_block(state: &State, enabled: bool) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    let chips = chips(state).iter().fold(row![].spacing(8), |line, label| {
        line.push(chip(label, chip_kind(label)))
    });
    let hint = tr(lang, "In the game: Insert or R3+L2\nopens the port's menu.").replace('\n', " ");
    let content = column![
        image(logo()).height(LOGO_HEIGHT),
        play_row(state, enabled),
        chips,
        text(quick_line(state)).size(13).style(text_muted),
        text(hint).size(12).style(text_muted),
    ]
    .spacing(14)
    .align_x(mirror_x(Right, state.rtl));
    let placed = container(dimmed(content.into(), state))
        .width(Fill)
        .height(Fill)
        .align_x(mirror_x(Right, state.rtl))
        .align_y(Bottom)
        .padding(mirror_padding(
            Padding {
                right: BLOCK_RIGHT,
                bottom: BLOCK_BOTTOM,
                ..Padding::ZERO
            },
            state.rtl,
        ));
    placed.into()
}

fn top_right(state: &State, enabled: bool) -> Element<'_, Message> {
    let mut line = row![].spacing(10).align_y(Center);
    // Right to left, the line is mirrored: the language button keeps the outer corner.
    if state.rtl {
        line = line.push(lang_menu::button_view(state));
    }
    match &state.update {
        UpdateState::Available(release) => {
            let pill = text(update_pill_text(&state.lang, &release.version)).size(11);
            line = line.push(
                button(pill)
                    .padding([3, 10])
                    .style(button_flat)
                    .on_press_maybe(enabled.then_some(Message::Update(UpdateMessage::Install))),
            );
        }
        // Not pressable while the package downloads.
        UpdateState::Downloading(release, percent) => {
            let shown = tr_fmt(
                &state.lang,
                "Downloading version {}… {}%",
                &[&release.version, &percent.to_string()],
            );
            line = line.push(
                button(text(shown).size(11))
                    .padding([3, 10])
                    .style(button_flat),
            );
        }
        _ => {}
    }
    if !state.rtl {
        line = line.push(lang_menu::button_view(state));
    }
    container(dimmed(line.into(), state))
        .width(Fill)
        .height(Fill)
        .align_x(mirror_x(Right, state.rtl))
        .align_y(Top)
        .padding(mirror_padding(
            Padding {
                top: TOP_RIGHT_TOP,
                right: TOP_RIGHT_RIGHT,
                ..Padding::ZERO
            },
            state.rtl,
        ))
        .into()
}

/// The interactive Home layer: corner monogram, update pill and language button, and the block
/// with the logotype, PLAY, chips, settings line and hint. Each part is sized to its content, so
/// the empty space between them lets clicks through to the layers below.
pub fn block(state: &State) -> Element<'_, Message> {
    let enabled = home_interactive_enabled(state);
    let mut layers = stack![].width(Fill).height(Fill);
    if state.splash.done {
        layers = layers.push(
            container(image(monogram()).width(MONOGRAM).height(MONOGRAM))
                .width(Fill)
                .height(Fill)
                .align_x(mirror_x(Left, state.rtl))
                .padding(MONOGRAM_EDGE),
        );
    }
    layers
        .push(top_right(state, enabled))
        .push(bottom_block(state, enabled))
        .into()
}
