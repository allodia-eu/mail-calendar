//! The count the reading pane states while several rows are picked (`docs/list-selection.md`,
//! rule 10).

use adw::prelude::*;
use mailcal_bindings::ViewMode;

use super::selection::SelectionSummary;
use crate::ui::icons;

/// What the reading pane says while several rows are picked: the count, over whatever the pane was
/// holding.
///
/// An overlay rather than another page of the detail stack, so the reading pane keeps its state:
/// dropping back to one selected row uncovers the message with no re-fetch, and the composer's
/// own swap of that stack is left alone.
pub(crate) struct SelectionCountPane {
    root: gtk::Box,
    label: gtk::Label,
}

impl SelectionCountPane {
    pub(crate) fn new() -> Self {
        let icon = gtk::Image::from_icon_name(icons::SELECTED_MAIL);
        icon.set_pixel_size(32);
        icon.add_css_class("dim-label");
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        label.set_justify(gtk::Justification::Center);
        label.add_css_class("title-3");
        label.add_css_class("dim-label");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_valign(gtk::Align::Center);
        root.set_halign(gtk::Align::Fill);
        root.set_vexpand(true);
        root.append(&icon);
        root.append(&label);
        // Opaque, or the message underneath shows through the words describing the selection.
        root.add_css_class("view");
        root.set_visible(false);
        Self { root, label }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows the count, or lets the pane go back to what it was holding.
    pub(crate) fn render(&self, summary: SelectionSummary, mode: &ViewMode) {
        let stated = summary.pane_label(mode);
        if let Some(text) = &stated {
            self.label.set_text(text);
        }
        self.root.set_visible(stated.is_some());
    }
}

#[cfg(test)]
#[path = "selection_pane_tests.rs"]
pub(super) mod tests;
