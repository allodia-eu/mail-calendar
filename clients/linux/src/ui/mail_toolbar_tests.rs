//! What belongs to the mail window's header bar, and what spans the window.

use adw::prelude::*;

use super::{MailToolbar, show_mail_chrome, window_frame};
use crate::ui::{AppInput, command_bar::CommandBarRow, search::SearchBar};

/// The header bar keeps the search field and the window's controls; New Mail has moved to the
/// command bar (`docs/command-bar.md`, rule 2).
pub(crate) fn search_and_the_window_controls_belong_to_the_header_bar() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let search = SearchBar::new(&sender);
    let toolbar = MailToolbar::new(search.entry());

    assert_eq!(
        toolbar.widget().title_widget().as_ref(),
        Some(search.entry().upcast_ref::<gtk::Widget>()),
        "the search field is the header bar's centred item"
    );
    assert!(toolbar.widget().shows_start_title_buttons());
    assert!(toolbar.widget().shows_end_title_buttons());
}

/// The header bar and the command bar each span the window, over every pane, rather than riding
/// one of them; and both are the mail surface's alone.
///
/// Asserted against the shell's own assembly rather than a copy of it: a rebuilt-in-the-test
/// arrangement would only prove that the test can build one.
pub(crate) fn the_header_and_the_command_bar_span_every_pane() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let search = SearchBar::new(&sender);
    let header = MailToolbar::new(search.entry()).widget().clone();
    let bar = CommandBarRow::new(&sender);
    let panes = gtk::Paned::new(gtk::Orientation::Horizontal);
    panes.set_start_child(Some(&gtk::Label::new(Some("folders"))));
    panes.set_end_child(Some(&gtk::Label::new(Some("list and reading"))));
    let (frame, band) = window_frame(&header, bar.widget(), &panes);

    for widget in [
        header.upcast_ref::<gtk::Widget>(),
        bar.widget().upcast_ref::<gtk::Widget>(),
    ] {
        assert!(
            widget.ancestor(gtk::Paned::static_type()).is_none(),
            "inside a pane, it is as narrow as that pane"
        );
        assert_eq!(
            widget.parent().as_ref(),
            Some(band.upcast_ref::<gtk::Widget>()),
            "one band, nothing between the two rows"
        );
    }
    assert_eq!(
        band.ancestor(adw::ToolbarView::static_type()).as_ref(),
        Some(frame.upcast_ref::<gtk::Widget>())
    );
    assert!(
        band.has_css_class("sidebar-pane"),
        "the band is drawn in the sidebar's own surface, the toolkit's colours"
    );
    assert_eq!(frame.content().as_ref(), Some(panes.upcast_ref()));

    show_mail_chrome(&band, false);
    assert!(!band.is_visible());
    show_mail_chrome(&band, true);
    assert!(band.is_visible());
}
