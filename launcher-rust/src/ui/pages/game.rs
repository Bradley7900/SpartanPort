//! The Game page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Game patches of the current dump; Task 26 fills it.
#[derive(Debug, Clone, Default)]
pub struct PatchesView;

/// Placeholder body; Task 26 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Game").into()
}
