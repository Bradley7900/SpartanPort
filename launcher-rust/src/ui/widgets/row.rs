//! A setting: its name on the left, the control on the right, an optional note below; mirrored
//! right to left (spec 4.5): the name on the right, the control before it.

use iced::widget::{column, container, row as layout, text};
use iced::{Center, Element, Fill, Right};

use crate::ui::app::Message;
use crate::ui::theme;

/// Width of the name column.
pub const LABEL_WIDTH: f32 = 220.0;

pub fn row<'a>(
    rtl: bool,
    label: &str,
    control: Element<'a, Message>,
    hint: Option<&str>,
) -> Element<'a, Message> {
    let mut body = if rtl {
        // Against the right edge, the name at the right of its column. The text keeps its own
        // alignment: an explicit one hides Arabic text in iced 0.14.
        let name = container(text(label.to_string()))
            .width(LABEL_WIDTH)
            .align_x(Right);
        let line = layout![control, name].spacing(18).align_y(Center);
        column![container(line).width(Fill).align_x(Right)]
            .spacing(2)
            .align_x(Right)
    } else {
        let name = text(label.to_string()).width(LABEL_WIDTH);
        let line = layout![name, control].spacing(18).align_y(Center);
        column![line].spacing(2)
    };
    if let Some(hint) = hint.filter(|hint| !hint.is_empty()) {
        body = body.push(text(hint.to_string()).size(13).style(theme::text_muted));
    }
    body.padding([4, 0]).into()
}
