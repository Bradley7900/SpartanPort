//! The Performance page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Placeholder body; Task 25 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Performance").into()
}
