//! The drawer: the panel that slides in from the left with the open page.

use iced::widget::{button, column, container, opaque, pin, row, scrollable, space, text};
use iced::{Center, Element, Fill};

use crate::core::i18n::tr;
use crate::ui::app::{Message, State};
use crate::ui::pages::{self, Page};
use crate::ui::persist::dirty;
use crate::ui::theme::{button_play, container_panel, text_muted, CINZEL_BLACK, CINZEL_BOLD};

/// Whether the drawer shows a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerState {
    #[default]
    Closed,
    Open(Page),
}

/// Share of the window width the drawer of `page` takes.
pub fn drawer_fraction(page: Page) -> f32 {
    if page.wide() {
        0.72
    } else {
        0.46
    }
}

/// Drawer width in logical pixels for the current page and window.
pub fn drawer_width(state: &State) -> f32 {
    state.window_size.width * drawer_fraction(state.page)
}

/// Horizontal shift of the drawer: `-width` when hidden (`progress` 0), none when open (1).
pub fn slide_offset(progress: f32, width: f32) -> f32 {
    (progress - 1.0) * width
}

/// The drawer of `page`, shifted by the slide animation at `state.now`.
pub fn view(state: &State, page: Page) -> Element<'_, Message> {
    let width = drawer_width(state);
    let progress = state.drawer_progress.interpolate(0.0, 1.0, state.now);
    let title = column![
        text(tr(&state.lang, page.label_en()))
            .font(CINZEL_BLACK)
            .size(30),
        // Subtitle placeholder; each page task supplies its own.
        text("…").style(text_muted),
    ]
    .spacing(4);
    let header = row![title, space::horizontal(), save_button(state)].align_y(Center);
    let body = scrollable(pages::view(state, page)).height(Fill);
    let panel = container(column![header, body].spacing(18))
        .padding(28)
        .width(width)
        .height(Fill)
        .style(container_panel);
    pin(opaque(panel)).x(slide_offset(progress, width)).into()
}

/// Save, at the right of the header: active only while the window differs from the files. The
/// state is worked out on each draw (`persist::dirty` compares two small maps, without I/O).
fn save_button(state: &State) -> Element<'_, Message> {
    button(text(tr(&state.lang, "Save")).font(CINZEL_BOLD).size(14))
        .padding([8, 22])
        .style(button_play)
        .on_press_maybe(dirty(state).then_some(Message::Save))
        .into()
}
