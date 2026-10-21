//! A small status pill.

use iced::widget::{container, text};
use iced::{Border, Element};

use crate::ui::app::Message;
use crate::ui::theme::palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Ok,
    Warn,
    Plain,
}

pub fn chip<'a>(label: &str, kind: ChipKind) -> Element<'a, Message> {
    let edge = match kind {
        ChipKind::Ok => palette::GOLD,
        ChipKind::Warn => palette::RED_HI,
        ChipKind::Plain => palette::LINE,
    };
    container(text(label.to_string()).size(12).color(palette::SILVER))
        .padding([2, 8])
        .style(move |_| container::Style {
            border: Border {
                color: edge,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}
