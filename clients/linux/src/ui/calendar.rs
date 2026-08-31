//! Linux calendar state and GTK rendering.

pub(super) mod attendees;
pub(in crate::ui) mod date;
mod dialogs;
mod dialogs_series;
mod drag;
mod editor;
mod grid;
mod manager;
mod model;
pub(in crate::ui) mod paint;
mod pane;
mod reference;
mod repeat;
mod views;

pub(crate) use drag::CreateSlot;
pub(crate) use editor::EventForm;
#[cfg(test)]
pub(crate) use grid::widget_tests;
pub(crate) use model::{CalendarMode, CalendarModel, EventIdentity};
pub(crate) use pane::CalendarPane;

#[cfg(test)]
#[path = "calendar/dialogs_series_widget_tests.rs"]
pub(crate) mod dialog_tests;
