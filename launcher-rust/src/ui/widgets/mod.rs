//! Settings widgets shared by the pages.
pub mod button_icon;
pub mod checkbox;
pub mod chip;
pub mod folder;
pub mod modal;
pub mod row;
pub mod section;
pub mod select;
pub mod slider;

pub use button_icon::{button_icon, spec, IconKind, IconSpec};
pub use checkbox::{checkbox, checkbox_ini};
pub use chip::{chip, ChipKind};
pub use folder::folder;
pub use modal::{modal, Modal, ModalKind, ModalMessage};
pub use row::row;
pub use section::section;
pub use select::{choice_pairs, select, select_message, select_pairs};
pub use slider::slider;
