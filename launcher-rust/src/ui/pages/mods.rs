//! The Mods page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Mod order, switches, conflicts and presets; Task 29 fills it.
#[derive(Debug, Clone, Default)]
pub struct ModsView;

/// Mods page events; Task 29 adds them.
#[derive(Debug, Clone)]
pub enum ModsMessage {}

/// Placeholder body; Task 29 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Mods").into()
}
