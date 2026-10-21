//! The Graphics page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Placeholder body; Task 24 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Graphics").into()
}
