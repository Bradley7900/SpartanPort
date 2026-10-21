//! The Trophies page.

use iced::widget::text;
use iced::Element;

use crate::ui::app::{Message, State};

/// Trophy icons, tiers, unlocked set and filter; Task 27 fills it.
#[derive(Debug, Clone, Default)]
pub struct TrophiesView;

/// Trophies page events; Task 27 adds them.
#[derive(Debug, Clone)]
pub enum TrophiesMessage {}

/// Placeholder body; Task 27 builds the page.
pub fn view(_state: &State) -> Element<'_, Message> {
    text("Trophies").into()
}
