//! Linux contacts state and GTK rendering.

mod dialog;
mod editor;
mod model;
pub(crate) mod pane;

pub(crate) use editor::EditTarget;
pub(crate) use model::{ContactsModel, PersonRow};
pub(crate) use pane::ContactsPane;
