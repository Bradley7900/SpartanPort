//! The Controls page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Controls page events; Task 28 adds them.
#[derive(Debug, Clone)]
pub enum ControlsMessage {}

/// Placeholder body; Task 28 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Controls").into()
}
