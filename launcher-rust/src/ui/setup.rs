//! The first-run setup (setup spec): three steps between the splash and the Home, shown while
//! `State::setup` is `Some`.

use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Background, Center, Element, Fill, Left, Task};

use crate::core::game::{
    missing_game_files, same_folder, REQUIRED_GAME_FILES, REQUIRED_UPDATE_FILES,
};
use crate::core::i18n::{tr, tr_fmt};
use crate::ui::app::{Message, State};
use crate::ui::home;
use crate::ui::pages::game::folder_row;
use crate::ui::pages::performance::select_row;
use crate::ui::persist::{self, Persisted};
use crate::ui::play::describe_missing;
use crate::ui::splash::SPLASH_BG;
use crate::ui::theme::{self, mirror_x, palette, CINZEL_BOLD};
use crate::ui::widgets::row as labeled_row;

/// One step of the setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Folders,
    Gpu,
    Video,
}

impl Step {
    /// Every step, in the order they are shown.
    pub const ALL: [Step; 3] = [Step::Folders, Step::Gpu, Step::Video];

    /// The position of the step, counting from 1.
    pub fn number(self) -> usize {
        match self {
            Step::Folders => 1,
            Step::Gpu => 2,
            Step::Video => 3,
        }
    }

    /// The step after this one, or `None` on the last.
    pub fn next(self) -> Option<Step> {
        match self {
            Step::Folders => Some(Step::Gpu),
            Step::Gpu => Some(Step::Video),
            Step::Video => None,
        }
    }

    /// The step before this one, or `None` on the first.
    pub fn previous(self) -> Option<Step> {
        match self {
            Step::Folders => None,
            Step::Gpu => Some(Step::Folders),
            Step::Video => Some(Step::Gpu),
        }
    }

    /// The English title of the step, the key of its translation.
    pub fn title_en(self) -> &'static str {
        match self {
            Step::Folders => "Folders",
            Step::Gpu => "Graphics card",
            Step::Video => "Video",
        }
    }

    /// The English sentence that introduces the step, the key of its translation.
    pub fn intro_en(self) -> &'static str {
        match self {
            Step::Folders => "Choose the folder of your game dump. The update folder is optional.",
            Step::Gpu => "The game runs on this graphics card.",
            Step::Video => {
                "Resolution and frame rate. The in-game menu (Insert or R3+L2) changes the same values."
            }
        }
    }
}

/// The setup while it is open.
#[derive(Debug, Clone)]
pub struct SetupState {
    /// The step on screen.
    pub step: Step,
    /// What the launcher would save as it stands when the setup opened.
    pub defaults: Persisted,
}

impl SetupState {
    /// A setup on its first step, with the current values of `state` as the defaults.
    pub fn new(state: &State) -> SetupState {
        SetupState {
            step: Step::Folders,
            defaults: persist::gather(state),
        }
    }
}

/// What the setup screen can ask for.
#[derive(Debug, Clone)]
pub enum SetupMessage {
    Next,
    Back,
    Skip,
    Finish,
}

/// The red lines under the folder `key` (`game_dir` or `update_dir`); empty when the folder passes.
pub fn folder_problems(state: &State, key: &str) -> Vec<String> {
    let lang = state.lang.as_str();
    let game_dir = state.settings.get_str("game_dir");
    let heading = |missing: &[_]| {
        let mut lines = vec![tr(lang, "Missing or empty files:")];
        lines.extend(describe_missing(lang, missing));
        lines
    };
    match key {
        "game_dir" => {
            let missing = missing_game_files(&game_dir, REQUIRED_GAME_FILES);
            if missing.is_empty() {
                Vec::new()
            } else {
                heading(&missing)
            }
        }
        "update_dir" => {
            let update_dir = state.settings.get_str("update_dir");
            if update_dir.is_empty() {
                return Vec::new();
            }
            if !game_dir.is_empty() && same_folder(&update_dir, &game_dir) {
                return vec![tr(
                    lang,
                    "Choose two separate folders for the game and its update, or leave the update folder empty.",
                )];
            }
            let missing = missing_game_files(&update_dir, REQUIRED_UPDATE_FILES);
            if missing.is_empty() {
                Vec::new()
            } else {
                heading(&missing)
            }
        }
        _ => Vec::new(),
    }
}

/// Whether Next may leave the current step: both folders pass on Folders, always on the others.
pub fn can_advance(state: &State) -> bool {
    match state.setup.as_ref().map(|setup| setup.step) {
        None => false,
        Some(Step::Folders) => {
            folder_problems(state, "game_dir").is_empty()
                && folder_problems(state, "update_dir").is_empty()
        }
        Some(_) => true,
    }
}

/// Applies one setup message to `state`.
pub fn update(state: &mut State, message: SetupMessage) -> Task<Message> {
    if matches!(message, SetupMessage::Next) && !can_advance(state) {
        return Task::none();
    }
    let Some(setup) = state.setup.as_mut() else {
        return Task::none();
    };
    match message {
        SetupMessage::Next => {
            if let Some(step) = setup.step.next() {
                setup.step = step;
            }
        }
        SetupMessage::Back => {
            if let Some(step) = setup.step.previous() {
                setup.step = step;
            }
        }
        SetupMessage::Skip | SetupMessage::Finish => {}
    }
    Task::none()
}

/// Width of the card.
const CARD_WIDTH: f32 = 640.0;

/// The splash's dark background, with the fire over it once the splash is gone.
pub fn background(state: &State) -> Element<'_, Message> {
    stack![
        container(space())
            .width(Fill)
            .height(Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(SPLASH_BG)),
                ..container::Style::default()
            }),
        state.fire.view(),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Corner monogram, language button and the centred card of the current step.
pub fn view(state: &State) -> Element<'_, Message> {
    let centred = container(card(state))
        .width(Fill)
        .height(Fill)
        .align_x(Center)
        .align_y(Center);
    stack![
        home::corner_monogram(state),
        home::top_right(state, false),
        centred
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// The two folder rows, each followed by its red problem lines.
fn folders_body(state: &State) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    let rows = [
        (
            "game_dir",
            tr(lang, "Game folder"),
            tr(
                lang,
                "Your own dump of the game (eboot.bin, sce_module, sce_sys): SpartanPort, any region and version.",
            ),
        ),
        (
            "update_dir",
            tr(lang, "Update folder (optional)"),
            tr(
                lang,
                "Only if the update is dumped apart from the game: it is laid over the game folder without changing either. Leave empty when the game folder already has the update.",
            ),
        ),
    ];
    let mut body = column![]
        .spacing(8)
        .width(Fill)
        .align_x(mirror_x(Left, state.rtl));
    for (key, label, hint) in rows {
        body = body.push(folder_row(state, key, &label, &hint));
        for line in folder_problems(state, key) {
            body = body.push(text(line).size(12).color(palette::RED_HI));
        }
    }
    body.into()
}

/// The graphics card selector, with a note while the cards have not been listed.
fn gpu_body(state: &State) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    let hint = tr(
        lang,
        "The GPU the game runs on. Choose your dedicated card if the game uses the integrated one.",
    );
    let mut body = column![labeled_row(
        state.rtl,
        &tr(lang, "Graphics card"),
        select_row(state, lang, "gpu_id"),
        Some(&hint),
    )]
    .spacing(8)
    .width(Fill)
    .align_x(mirror_x(Left, state.rtl));
    if state.gpus.is_empty() {
        body = body.push(
            text(tr(lang, "Graphics card not checked: the game picks one."))
                .size(13)
                .style(theme::text_muted),
        );
    }
    body.into()
}

/// The card of the step on screen: title, step line, intro, body and footer.
fn card(state: &State) -> Element<'_, Message> {
    let lang = state.lang.as_str();
    let step = state
        .setup
        .as_ref()
        .map_or(Step::Folders, |setup| setup.step);
    let number = step.number().to_string();
    let step_line = format!(
        "{} · {}",
        tr_fmt(lang, "Step {} of {}", &[&number, "3"]),
        tr(lang, step.title_en())
    );
    let body: Element<'_, Message> = match step {
        Step::Folders => folders_body(state),
        Step::Gpu => gpu_body(state),
        Step::Video => column![].into(),
    };
    // Left to right: Skip at the start, then Back and Next or Finish; right to left the row is
    // reversed, so Skip is at the right.
    let mut buttons: Vec<Element<'_, Message>> = vec![
        button(text(tr(lang, "Skip")).style(theme::text_muted))
            .style(theme::button_flat)
            .on_press(Message::Setup(SetupMessage::Skip))
            .into(),
        space().width(Fill).into(),
    ];
    if step.previous().is_some() {
        buttons.push(
            button(text(tr(lang, "Back")))
                .style(theme::button_flat)
                .on_press(Message::Setup(SetupMessage::Back))
                .into(),
        );
    }
    buttons.push(if step.next().is_some() {
        button(text(tr(lang, "Next")))
            .style(theme::button_flat)
            .on_press_maybe(can_advance(state).then_some(Message::Setup(SetupMessage::Next)))
            .into()
    } else {
        button(text(tr(lang, "Finish")))
            .style(theme::button_play)
            .on_press(Message::Setup(SetupMessage::Finish))
            .into()
    });
    if state.rtl {
        buttons.reverse();
    }
    let footer = row(buttons).spacing(8);
    container(
        column![
            text(tr(lang, "Initial setup")).font(CINZEL_BOLD).size(22),
            text(step_line).style(theme::text_muted),
            text(tr(lang, step.intro_en())).style(theme::text_muted),
            body,
            footer,
            text(tr(
                lang,
                "You can change all of this later in the Graphics, Performance and Game tabs."
            ))
            .style(theme::text_muted),
        ]
        .spacing(14)
        .width(Fill)
        .align_x(mirror_x(Left, state.rtl)),
    )
    .padding(24)
    .width(CARD_WIDTH)
    .style(theme::container_card)
    .into()
}
