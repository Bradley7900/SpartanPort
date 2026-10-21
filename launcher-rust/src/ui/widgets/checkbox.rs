//! Check boxes: one stores a JSON bool, the other the ini's `"1"` / `"0"`.

use iced::widget::{checkbox as check_box, column, text};
use iced::Element;
use serde_json::Value;

use crate::ui::app::Message;
use crate::ui::theme;

fn with_hint<'a>(
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
    value: impl Fn(bool) -> Value + 'a,
) -> Element<'a, Message> {
    let key = key.to_string();
    let boxed = check_box(checked)
        .label(label.to_string())
        .on_toggle(move |on| Message::Set(key.clone(), value(on)))
        .style(theme::checkbox_red);
    let mut body = column![boxed].spacing(2);
    if let Some(hint) = hint.filter(|hint| !hint.is_empty()) {
        body = body.push(text(hint.to_string()).size(13).style(theme::text_muted));
    }
    body.padding([4, 0]).into()
}

pub fn checkbox<'a>(
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
) -> Element<'a, Message> {
    with_hint(key, label, checked, hint, Value::Bool)
}

pub fn checkbox_ini<'a>(
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
) -> Element<'a, Message> {
    with_hint(key, label, checked, hint, |on| {
        Value::String(if on { "1" } else { "0" }.to_string())
    })
}
