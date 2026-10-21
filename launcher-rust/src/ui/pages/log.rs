//! The Log page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Placeholder body; Task 31 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Log").into()
}
