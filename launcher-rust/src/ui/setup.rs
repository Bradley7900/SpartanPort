//! The first-run setup (setup spec): three steps between the splash and the Home, shown while
//! `State::setup` is `Some`.

use iced::Task;

use crate::ui::app::{Message, State};
use crate::ui::persist::{self, Persisted};

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
