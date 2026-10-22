//! The version button of the Home (spec 4.5): the installed release, with the newer one noted.

use iced::time::Instant;
use iced::widget::{button, container, mouse_area, text};
use iced::{Element, Task};

use crate::core::i18n::tr_fmt;
use crate::ui::app::{home_interactive_enabled, home_interactive_opacity, Message, State};
use crate::ui::hover_menu::{self, HoverMessage};
use crate::ui::theme::{button_flat, faded_button};
use crate::ui::update::{self, UpdateMessage, UpdateState};

/// Id of the container that holds the button, for the tests.
pub const BUTTON_ID: &str = "version-button";

/// Button events.
#[derive(Debug, Clone)]
pub enum VersionMessage {
    Enter,
    Leave,
    Toggle,
    Tick(Instant),
}

/// Applies the hover timers; when the menu just opened over `UpdateState::None` or `Failed`,
/// returns the task of a silent check.
pub fn update(state: &mut State, message: VersionMessage) -> Task<Message> {
    let hover = match message {
        VersionMessage::Enter => HoverMessage::Enter,
        VersionMessage::Leave => HoverMessage::Leave,
        VersionMessage::Toggle => HoverMessage::Toggle,
        VersionMessage::Tick(at) => HoverMessage::Tick(at),
    };
    let was_open = state.version_menu.open;
    let now = state.now;
    hover_menu::apply(&mut state.version_menu, hover, now);
    let stale = matches!(state.update, UpdateState::None | UpdateState::Failed(_));
    if !was_open && state.version_menu.open && stale {
        return update::update(state, UpdateMessage::Check { manual: false });
    }
    Task::none()
}

/// Whether a timer is running, so the app keeps the clock ticking.
pub fn timing(state: &State) -> bool {
    hover_menu::timing(&state.version_menu)
}

/// The installed release, with the newer stable noted; the download's progress while one runs.
pub fn button_text(state: &State) -> String {
    let lang = state.lang.as_str();
    if let UpdateState::Downloading {
        release, percent, ..
    } = &state.update
    {
        return tr_fmt(
            lang,
            "Downloading version {}… {}%",
            &[&release.id.short(), &percent.to_string()],
        );
    }
    let mut label = state.installed.label(lang);
    if let Some(newer) = update::newer(state) {
        label.push_str(" · ");
        label.push_str(&tr_fmt(lang, "{} available", &[&newer.id.version]));
    }
    label.push_str(" ▾");
    label
}

/// The button, before the language one in the top-right corner.
pub fn button_view(state: &State) -> Element<'_, Message> {
    let downloading = matches!(state.update, UpdateState::Downloading { .. });
    let enabled = home_interactive_enabled(state);
    let opacity = home_interactive_opacity(state);
    let pressed = button(text(button_text(state)).size(12))
        .padding([3, 10])
        .style(move |theme, status| faded_button(button_flat(theme, status), opacity))
        .on_press_maybe(
            (enabled && !downloading).then_some(Message::VersionMenu(VersionMessage::Toggle)),
        );
    // The id sits on a container: iced's button has none.
    let pressed = container(pressed).id(BUTTON_ID);
    if !enabled {
        return pressed.into();
    }
    mouse_area(pressed)
        .on_enter(Message::VersionMenu(VersionMessage::Enter))
        .on_exit(Message::VersionMenu(VersionMessage::Leave))
        .into()
}
