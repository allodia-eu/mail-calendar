//! Linux calendar state and GTK rendering.

mod answer;
pub(super) mod attendees;
mod conference;
pub(in crate::ui) mod date;
mod detail;
mod dialogs;
mod dialogs_series;
mod drag;
mod editor;
mod editor_meeting;
mod grid;
pub(super) mod invitees_editor;
mod manager;
mod meeting;
pub(super) mod meeting_row;
mod model;
pub(in crate::ui) mod paint;
mod pane;
mod reference;
mod repeat;
mod repeat_editor;
mod views;

pub(crate) use drag::CreateSlot;
pub(crate) use editor::EventForm;
#[cfg(test)]
pub(crate) use grid::widget_tests;
pub(crate) use model::{CalendarMode, CalendarModel, EventIdentity};
pub(crate) use pane::CalendarPane;

#[cfg(test)]
#[path = "calendar/detail_widget_tests.rs"]
pub(crate) mod detail_tests;
#[cfg(test)]
#[path = "calendar/dialogs_series_widget_tests.rs"]
pub(crate) mod dialog_tests;
#[cfg(test)]
#[path = "calendar/views_widget_tests.rs"]
pub(crate) mod views_tests;
