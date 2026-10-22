//! Check boxes as setting rows (mockup `.row` with `.cb`): the name, then the box at the row's end;
//! the name toggles it too. One stores a JSON bool, the other the ini's `"1"` / `"0"`.

use iced::widget::{checkbox as check_box, mouse_area, text};
use iced::Element;
use serde_json::Value;

use crate::ui::app::Message;
use crate::ui::theme;
use crate::ui::widgets::row::line;

/// A check box row: `on_toggle` makes the message of a new state; without it the box is drawn but
/// nothing toggles it.
pub fn toggle_row<'a, F>(
    rtl: bool,
    label: &str,
    checked: bool,
    on_toggle: Option<F>,
    hint: Option<&str>,
) -> Element<'a, Message>
where
    F: Fn(bool) -> Message + 'a,
{
    let boxed = check_box(checked).style(theme::checkbox_red);
    let (name, boxed): (Element<'a, Message>, _) = match on_toggle {
        Some(toggle) => (
            mouse_area(text(label.to_string()))
                .on_press(toggle(!checked))
                .into(),
            boxed.on_toggle(toggle),
        ),
        None => (text(label.to_string()).into(), boxed),
    };
    line(rtl, name, label, boxed.into(), hint)
}

fn setting<'a>(
    rtl: bool,
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
    value: impl Fn(bool) -> Value + 'a,
) -> Element<'a, Message> {
    let key = key.to_string();
    let toggle = move |on| Message::Set(key.clone(), value(on));
    toggle_row(rtl, label, checked, Some(toggle), hint)
}

pub fn checkbox<'a>(
    rtl: bool,
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
) -> Element<'a, Message> {
    setting(rtl, key, label, checked, hint, Value::Bool)
}

pub fn checkbox_ini<'a>(
    rtl: bool,
    key: &str,
    label: &str,
    checked: bool,
    hint: Option<&str>,
) -> Element<'a, Message> {
    setting(rtl, key, label, checked, hint, |on| {
        Value::String(if on { "1" } else { "0" }.to_string())
    })
}
