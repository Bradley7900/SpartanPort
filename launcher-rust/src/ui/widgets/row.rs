//! A setting: its name on the left, the control at the row's right end, an optional note below
//! across the row (mockup `.row { grid-template-columns: 1fr auto }`); mirrored right to left
//! (spec 4.5): the control at the left end, the name against the right edge.

use iced::widget::{column, container, row as layout, text, Id};
use iced::{Center, Element, Fill, Right};

use crate::ui::app::Message;
use crate::ui::theme;

/// Space between the name and the control (mockup `.row` gap).
pub const GAP: f32 = 14.0;

/// The id of the box that holds the control of the row named `label`.
pub fn control_id(label: &str) -> Id {
    Id::from(format!("row-control: {label}"))
}

/// A row named by the text `label`.
pub fn row<'a>(
    rtl: bool,
    label: &str,
    control: Element<'a, Message>,
    hint: Option<&str>,
) -> Element<'a, Message> {
    line(rtl, text(label.to_string()).into(), label, control, hint)
}

/// A row named by `name` (a check box's name, which toggles it too); `label` is its text, which
/// names the control's box.
///
/// The control keeps its own width and the name takes what is left, so a narrow drawer wraps the
/// name, never the control.
pub fn line<'a>(
    rtl: bool,
    name: Element<'a, Message>,
    label: &str,
    control: Element<'a, Message>,
    hint: Option<&str>,
) -> Element<'a, Message> {
    let control = container(control).id(control_id(label));
    let mut body = if rtl {
        // The text keeps its own alignment: an explicit one hides Arabic text in iced 0.14.
        let name = container(name).width(Fill).align_x(Right);
        column![layout![control, name].spacing(GAP).align_y(Center)]
            .spacing(2)
            .align_x(Right)
    } else {
        let name = container(name).width(Fill);
        column![layout![name, control].spacing(GAP).align_y(Center)].spacing(2)
    };
    if let Some(hint) = hint.filter(|hint| !hint.is_empty()) {
        let note = text(hint.to_string()).size(13).style(theme::text_muted);
        // Left to right the note spans the row from the name's edge.
        body = body.push(if rtl { note } else { note.width(Fill) });
    }
    body.padding([4, 0]).into()
}
