//! The first-run setup (setup spec): three steps between the splash and the Home, shown while
//! `State::setup` is `Some`.

use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Background, Center, Element, Fill, Left, Task};

use crate::core::i18n::{tr, tr_fmt};
use crate::ui::app::{Message, State};
use crate::ui::home;
use crate::ui::persist::{self, Persisted};
use crate::ui::splash::SPLASH_BG;
use crate::ui::theme::{self, mirror_x, CINZEL_BOLD};

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

/// Applies one setup message to `state`.
pub fn update(state: &mut State, message: SetupMessage) -> Task<Message> {
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
    let body = column![];
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
            .on_press(Message::Setup(SetupMessage::Next))
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
