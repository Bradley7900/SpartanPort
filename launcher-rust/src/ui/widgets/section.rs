//! A section heading with a line under it.

use iced::widget::{column, container, space, text};
use iced::{Background, Element, Fill};

use crate::ui::app::Message;
use crate::ui::theme::{palette, CINZEL_BOLD};

pub fn section<'a>(title: &str) -> Element<'a, Message> {
    let heading = text(title.to_uppercase())
        .font(CINZEL_BOLD)
        .size(16)
        .color(palette::RED_HI);
    let rule = container(space())
        .width(Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(Background::Color(palette::LINE)),
            ..container::Style::default()
        });
    column![heading, rule].spacing(4).padding([12, 0]).into()
}
