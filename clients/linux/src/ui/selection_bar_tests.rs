//! What the mail actions bar must do, and what it must never do.
//!
//! The GTK halves are called from the crate's single `gtk::init` test (see
//! [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::{BulkAction, ViewMode};

use super::{
    CLEAR_ICON, SELECT_ALL_ICON, SYNC_ICON, SelectionBar, SelectionCountPane, action_icon,
    mail_surface,
};
use crate::ui::{AppInput, selection::SelectionSummary};

/// A named glyph the theme lacks draws the broken-image icon, and the bar keeps drawing as though
/// nothing happened; so the names are asserted rather than looked at once.
pub(crate) fn every_action_icon_resolves_to_a_real_glyph() {
    let display = gtk::gdk::Display::default().expect("a display");
    let theme = gtk::IconTheme::for_display(&display);
    let icons = [
        action_icon(BulkAction::MarkRead),
        action_icon(BulkAction::MarkUnread),
        action_icon(BulkAction::Flag),
        action_icon(BulkAction::Unflag),
        action_icon(BulkAction::Archive),
        action_icon(BulkAction::Delete),
        action_icon(BulkAction::PermanentlyDelete),
        SELECT_ALL_ICON,
        CLEAR_ICON,
        SYNC_ICON,
    ];
    for icon in icons {
        assert!(theme.has_icon(icon), "the bar must be able to draw {icon}");
    }
    assert!(
        !theme.has_icon("mailcal-not-an-icon-symbolic"),
        "a theme that answers yes to everything would make the check above meaningless"
    );
}

/// The bar stands with nothing selected, and says so by what can be pressed rather than by
/// appearing (`docs/list-selection.md`, rule 5). Select all is the exception: it is how a
/// pointer starts a selection without touching a modifier.
pub(crate) fn an_empty_selection_disables_only_the_actions_that_need_a_selection() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let bar = SelectionBar::new(&sender);

    bar.render(SelectionSummary {
        count: 0,
        any_unread: false,
        any_unflagged: false,
    });
    assert!(
        bar.widget().is_visible(),
        "the bar is chrome, not something that arrives with the first click"
    );
    assert!(
        bar.needs_selection
            .iter()
            .all(|button| !button.is_sensitive()),
        "an action over no rows must not be pressable"
    );

    bar.render(SelectionSummary {
        count: 2,
        any_unread: true,
        any_unflagged: true,
    });
    assert!(
        bar.needs_selection
            .iter()
            .all(gtk::prelude::WidgetExt::is_sensitive),
        "a selection makes every action live"
    );
}

/// Sync acts on the mailbox rather than the selection, so it stays live and is separated from
/// the controls whose sensitivity follows the selected rows.
pub(crate) fn sync_is_after_the_selection_actions_and_always_live() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let bar = SelectionBar::new(&sender);

    bar.render(SelectionSummary {
        count: 0,
        any_unread: false,
        any_unflagged: false,
    });
    assert!(bar.sync.is_sensitive());
    bar.sync.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::RefreshRequested)
    ));

    let previous = bar.sync.prev_sibling().expect("Sync follows a divider");
    assert!(
        previous.downcast_ref::<gtk::Separator>().is_some(),
        "Sync must sit past a divider rather than among the selection actions"
    );
}

/// The count belongs to the reading pane, and only above one row: at one, that row is the message
/// the pane is showing.
pub(crate) fn the_pane_states_the_count_only_while_it_covers_something_worth_covering() {
    let pane = SelectionCountPane::new();
    let summary = |count: usize| SelectionSummary {
        count,
        any_unread: false,
        any_unflagged: false,
    };

    pane.render(summary(0), &ViewMode::Threaded);
    assert!(!pane.widget().is_visible());
    pane.render(summary(1), &ViewMode::Threaded);
    assert!(
        !pane.widget().is_visible(),
        "one selected row is the one the pane is already showing"
    );
    pane.render(summary(2), &ViewMode::Threaded);
    assert!(pane.widget().is_visible());
    pane.render(summary(0), &ViewMode::Threaded);
    assert!(
        !pane.widget().is_visible(),
        "clearing the selection gives the pane back what it was holding"
    );
}

/// The bar spans the split rather than living in the list pane, which is what keeps its buttons
/// off the horizontal scrollbar a narrow list column gave them.
///
/// Asserted against the shell's own assembly rather than a copy of it: a rebuilt-in-the-test
/// arrangement would only prove that the test can build one.
pub(crate) fn the_bar_spans_both_panes_rather_than_riding_the_list() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let bar = SelectionBar::new(&sender);
    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_start_child(Some(&gtk::Label::new(Some("list"))));
    split.set_end_child(Some(&gtk::Label::new(Some("reading"))));
    let surface = mail_surface(&bar, &split);

    let widget = bar.widget().clone().upcast::<gtk::Widget>();
    assert!(
        widget.ancestor(gtk::Paned::static_type()).is_none(),
        "inside the split, the bar is as narrow as whichever pane holds it"
    );
    assert_eq!(
        widget.parent().as_ref(),
        Some(surface.upcast_ref::<gtk::Widget>()),
        "the bar is the surface's own first child, over both panes"
    );
    assert_eq!(
        split.parent().as_ref(),
        Some(surface.upcast_ref::<gtk::Widget>()),
        "and the split is the second, taking the height the bar leaves"
    );
}
