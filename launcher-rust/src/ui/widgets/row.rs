//! A setting: its name on the left, the control on the right, an optional note below.

use iced::widget::{column, row as layout, text};
use iced::{Center, Element};

use crate::ui::app::Message;
use crate::ui::theme;

/// Width of the name column.
pub const LABEL_WIDTH: f32 = 220.0;

pub fn row<'a>(
    label: &str,
    control: Element<'a, Message>,
    hint: Option<&str>,
) -> Element<'a, Message> {
    let line = layout![text(label.to_string()).width(LABEL_WIDTH), control]
        .spacing(18)
        .align_y(Center);
    let mut body = column![line].spacing(2);
    if let Some(hint) = hint.filter(|hint| !hint.is_empty()) {
        body = body.push(text(hint.to_string()).size(13).style(theme::text_muted));
    }
    body.padding([4, 0]).into()
}
