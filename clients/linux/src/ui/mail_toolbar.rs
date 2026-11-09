//! The mail window's header bar and the frame that stands it, with the command bar, over every
//! pane (`docs/command-bar.md`, rule 2).

use adw::prelude::*;

pub(crate) struct MailToolbar {
    root: adw::HeaderBar,
}

impl MailToolbar {
    /// The search field, centred, and the window's own controls at both ends: this bar spans the
    /// window, so its corners are the window's.
    pub(crate) fn new(search: &gtk::SearchEntry) -> Self {
        let root = adw::HeaderBar::new();
        root.set_show_start_title_buttons(true);
        root.set_show_end_title_buttons(true);
        root.set_title_widget(Some(search));
        Self { root }
    }

    pub(crate) fn widget(&self) -> &adw::HeaderBar {
        &self.root
    }
}

/// The window's frame: the mail header bar and the command bar, both spanning the window over
/// `panes`, and the band they make. Both are the mail surface's, so [`show_mail_chrome`] takes the
/// band away while the calendar or contacts, which draw their own, are in front.
///
/// The two rows are one band of chrome in the sidebar's own surface, with nothing between them, so
/// the band and the folder pane frame the list and the reading pane, which keep the content
/// surface. The surface is libadwaita's `sidebar-pane`, so dark mode and a window in the
/// background follow the toolkit.
pub(crate) fn window_frame(
    header: &adw::HeaderBar,
    bar: &impl IsA<gtk::Widget>,
    panes: &impl IsA<gtk::Widget>,
) -> (adw::ToolbarView, gtk::Box) {
    let band = gtk::Box::new(gtk::Orientation::Vertical, 0);
    band.add_css_class("sidebar-pane");
    band.append(header);
    band.append(bar);
    let frame = adw::ToolbarView::new();
    frame.add_top_bar(&band);
    frame.set_content(Some(panes));
    (frame, band)
}

pub(crate) fn show_mail_chrome(band: &gtk::Box, mail: bool) {
    band.set_visible(mail);
}

#[cfg(test)]
#[path = "mail_toolbar_tests.rs"]
pub(super) mod tests;
