//! What a redraw of the open Settings window keeps: where the page was scrolled to, and anything
//! the person is typing when the core's settings signal asks for one.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::AccountsSnapshot;

/// What to do with the core's settings signal while Accounts is on screen.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Signalled {
    Draw,
    /// The accounts are as drawn: the signal was about another page's setting.
    Skip,
    /// The person is typing; ask again on the next update.
    Wait,
}

/// The snapshot is read only when the answer depends on it.
pub(super) fn signalled(
    typing: bool,
    drawn: Option<&AccountsSnapshot>,
    now: impl FnOnce() -> AccountsSnapshot,
) -> Signalled {
    if typing {
        Signalled::Wait
    } else if drawn == Some(&now()) {
        Signalled::Skip
    } else {
        Signalled::Draw
    }
}

/// Whether a redraw would take something the person is typing: the caret is in a field, or a
/// field was edited since the window was drawn. The second holds after the caret moves on, which
/// is usually to the button that submits what was typed.
pub(super) fn typing(window: &gtk::Window, edited: bool) -> bool {
    edited
        || GtkWindowExt::focus(window)
            .is_some_and(|focus| focus.is::<gtk::Editable>() || focus.is::<gtk::TextView>())
}

/// Sets `edited` when the person changes any field under `root`. Connected after the page is
/// built, so the text a page is drawn with does not count.
pub(super) fn watch_edits(root: &gtk::Widget, edited: &Rc<Cell<bool>>) {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
            let edited = Rc::clone(edited);
            editable.connect_changed(move |_| edited.set(true));
        }
        watch_edits(&widget, edited);
        child = widget.next_sibling();
    }
}

/// Scrolls a newly built page to `offset`, before its first allocation: the viewport places its
/// child by the value it finds then, clamped to the height it has. A value set during that
/// allocation moves the scrollbar and leaves the page at the top.
///
/// The adjustment is given just enough range to hold the value, which a page with no height yet
/// would otherwise clamp to zero.
pub(super) fn keep_offset(scroll: &gtk::ScrolledWindow, offset: f64) {
    scroll
        .vadjustment()
        .configure(offset, 0.0, offset + 1.0, 0.0, 0.0, 1.0);
}

/// How far the page on screen is scrolled. A page not laid out yet already holds the offset it
/// was drawn to keep, so a second redraw in the same frame (a change made on the page, then the
/// core's signal about it) keeps it too.
pub(super) fn offset(pages: &gtk::Stack) -> Option<f64> {
    pages
        .visible_child()
        .and_downcast::<gtk::ScrolledWindow>()
        .map(|scroll| scroll.vadjustment().value())
}

#[cfg(test)]
#[path = "redraw_tests.rs"]
pub(crate) mod tests;
