#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The settings widgets: what they emit when the user acts on them.

use iced::Element;
use iced_test::simulator;
use serde_json::Value;
use spartanport_launcher::core::options::UPSCALERS;
use spartanport_launcher::ui::app::Message;
use spartanport_launcher::ui::widgets::modal::ModalMessage;
use spartanport_launcher::ui::widgets::{self, ModalKind};

/// Renders `view`, clicks the widget showing `label` and returns what it emitted.
fn click(view: Element<'_, Message>, label: &str) -> Vec<Message> {
    let mut ui = simulator(view);
    ui.click(label)
        .unwrap_or_else(|error| panic!("click {label:?}: {error:?}"));
    ui.into_messages().collect()
}

#[test]
fn select_emits_set_with_the_value_not_the_label() {
    // The simulator cannot open a pick_list overlay reliably, so the label -> value mapping the
    // widget's `on_select` runs is tested directly; the widget itself is built to prove it renders.
    let _view = widgets::select("upscaler", UPSCALERS, "fsr3", "en");
    let message = widgets::select_message(
        "upscaler",
        UPSCALERS,
        "en",
        "TAA (native resolution anti-aliasing)",
    );
    assert!(
        matches!(&message, Some(Message::Set(key, Value::String(value))) if key == "upscaler" && value == "taa"),
        "{message:?}"
    );
    let russian = widgets::select_message("upscaler", UPSCALERS, "ru", "Выключен");
    assert!(
        matches!(&russian, Some(Message::Set(_, Value::String(value))) if value == "off"),
        "{russian:?}"
    );
    assert!(widgets::select_message("upscaler", UPSCALERS, "en", "nope").is_none());
}

#[test]
fn checkbox_ini_emits_one_and_zero() {
    for (checked, expected) in [(true, "0"), (false, "1")] {
        let messages = click(
            widgets::checkbox_ini("vsync", "Vsync", checked, None),
            "Vsync",
        );
        assert_eq!(messages.len(), 1);
        assert!(
            matches!(&messages[0], Message::Set(key, Value::String(v)) if key == "vsync" && v == expected),
            "{checked}: {messages:?}"
        );
    }
    let messages = click(
        widgets::checkbox("fx", "Effects", false, Some("hint")),
        "Effects",
    );
    assert!(matches!(&messages[0], Message::Set(key, Value::Bool(true)) if key == "fx"));
}

#[test]
fn folder_browse_emits_browse_with_the_key() {
    let view = widgets::folder(false, "game_dir", "Game folder", "", "Not set", "en");
    let messages = click(view, "Browse…");
    assert!(
        matches!(&messages[0], Message::Browse(key) if key == "game_dir"),
        "{messages:?}"
    );
}

#[test]
fn modal_yes_no_emit_their_messages() {
    let yes_no = || ModalKind::YesNo {
        yes: Message::Save,
        no: Message::Stop,
    };
    let view = widgets::modal(yes_no(), "Title", "Body", "en");
    assert!(matches!(click(view, "Yes")[0], Message::Save));
    let view = widgets::modal(yes_no(), "Title", "Body", "en");
    assert!(matches!(click(view, "No")[0], Message::Stop));
    for kind in [ModalKind::Info, ModalKind::Error] {
        let view = widgets::modal(kind, "Title", "Body", "en");
        assert!(matches!(
            click(view, "OK")[0],
            Message::Modal(ModalMessage::Dismiss)
        ));
    }
}
