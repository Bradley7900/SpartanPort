//! A folder setting: the current path, `Browse…` and `Open`.

use std::path::PathBuf;

use iced::widget::{button, row as layout, text};
use iced::{Center, Element, Fill};

use crate::core::i18n::tr;
use crate::ui::app::Message;
use crate::ui::theme;
use crate::ui::widgets::row::row;

pub fn folder<'a>(
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
    let browse = button(text(tr(lang, "Browse…")))
        .style(theme::button_flat)
        .on_press(Message::Browse(key.to_string()));
    let open = button(text(tr(lang, "Open"))).style(theme::button_flat);
    let open = if value.is_empty() {
        open
    } else {
        open.on_press(Message::OpenPath(PathBuf::from(value)))
    };
    let control = layout![shown.width(Fill), browse, open]
        .spacing(6)
        .align_y(Center);
    row(label, control.into(), None)
}
