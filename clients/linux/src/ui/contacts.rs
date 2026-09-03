//! Linux contacts state and GTK rendering.

mod dialog;
mod editor;
mod model;
pub(crate) mod pane;

pub(crate) use editor::EditTarget;
pub(crate) use model::ContactsModel;
/// Named only by the showcase hook, which a release build without `dev-harness` compiles out.
#[cfg(any(debug_assertions, feature = "dev-harness"))]
pub(crate) use model::PersonRow;
pub(crate) use pane::ContactsPane;
