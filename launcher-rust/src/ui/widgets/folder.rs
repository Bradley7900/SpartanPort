//! A folder setting: the current path, `Browse…` and `Open`.

use std::path::PathBuf;

use iced::widget::text::Wrapping;
use iced::widget::{button, container, row as layout, text};
use iced::{Center, Element, Fill, Right};

use crate::core::i18n::tr;
use crate::ui::app::Message;
use crate::ui::theme;
use crate::ui::widgets::row::row;

pub fn folder<'a>(
    rtl: bool,
    key: &str,
    label: &str,
    value: &str,
    empty_hint: &str,
    lang: &str,
) -> Element<'a, Message> {
    let shown = if value.is_empty() {
        text(empty_hint.to_string()).style(theme::text_muted)
    } else {
        text(value.to_string())
    };
    // A long folder name breaks inside a word rather than run under the buttons.
    let shown = shown.wrapping(Wrapping::WordOrGlyph);
    let browse = button(text(tr(lang, "Browse…")))
        .style(theme::button_flat)
        .on_press(Message::Browse(key.to_string()));
    let open = button(text(tr(lang, "Open"))).style(theme::button_flat);
    let open = if value.is_empty() {
        open
    } else {
        open.on_press(Message::OpenPath(PathBuf::from(value)))
    };
    // Right to left the path sits next to the name, the buttons before it.
    let control = if rtl {
        let path = container(shown).width(Fill).align_x(Right);
        layout![open, browse, path]
    } else {
        layout![shown.width(Fill), browse, open]
    }
    .spacing(6)
    .align_y(Center);
    row(rtl, label, control.into(), None)
}
