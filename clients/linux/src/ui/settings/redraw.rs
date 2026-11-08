//! What a redraw of the open Settings window keeps: where the page was scrolled to, and anything
//! the person is typing when the core's settings signal asks for one.

use std::{cell::Cell, rc::Rc};

use adw::{glib, prelude::*};

/// What to do with the core's settings signal while Accounts is on screen.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Signalled<T> {
    /// Draw from what was just read, so the pages read nothing again.
    Draw(T),
    /// The accounts are as drawn: the signal was about another page's setting.
    Skip,
    /// The person is typing; ask again on the next update.
    Wait,
}

/// What Accounts is drawn from: every account, the open account's page, whose controls a
/// running change holds without changing any account, and the notices at that page's top.
pub(super) type Drawn = (
    mailcal_bindings::AccountsSnapshot,
    Option<mailcal_bindings::AccountPage>,
    Vec<mailcal_bindings::Notice>,
);

pub(super) fn drawn(app: &mailcal_bindings::MailcalApp, account: Option<&str>) -> Drawn {
    (
        app.accounts_snapshot(),
        account.and_then(|id| app.account_page(id.to_owned())),
        account.map_or_else(Vec::new, |id| super::account_notices::of(app, id)),
    )
}

/// What was drawn is read again only when the answer depends on it.
pub(super) fn signalled<T: PartialEq>(
    typing: bool,
    drawn: Option<&T>,
    now: impl FnOnce() -> T,
) -> Signalled<T> {
    if typing {
        return Signalled::Wait;
    }
    let now = now();
    if drawn == Some(&now) {
        Signalled::Skip
    } else {
        Signalled::Draw(now)
    }
}

/// Whether a redraw would take something the person is typing: the caret is in a field, or a
/// field was edited since the window was drawn. The second holds after the caret moves on, which
/// is usually to the button that submits what was typed. A field that sends what was typed
/// itself counts only while it holds text it has not sent.
pub(super) fn typing(window: &gtk::Window, edited: bool) -> bool {
    edited
        || GtkWindowExt::focus(window).is_some_and(|focus| {
            crate::ui::controls::unsent(&focus).unwrap_or_else(|| {
                focus.is::<gtk::Editable>() || focus.is::<gtk::TextView>()
            })
        })
}

/// A list open over the page, such as a picker's: a redraw now would close it under the pointer
/// and lose the pick.
pub(super) fn open_popover(root: &gtk::Widget) -> Option<gtk::Popover> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(popover) = widget
            .downcast_ref::<gtk::Popover>()
            .filter(|popover| popover.is_visible())
        {
            return Some(popover.clone());
        }
        if let Some(popover) = open_popover(&widget) {
            return Some(popover);
        }
        child = widget.next_sibling();
    }
    None
}

/// Whether a redraw waits for a list open over `window`, asked again once it closes: drawn now,
/// the list would close under the pointer, and the pick, which the core queues behind what runs,
/// would be lost. `waiting_on` keeps the list already watched, so a second signal while it is open
/// connects nothing more.
pub(super) fn wait_for_open_list(
    window: Option<&gtk::Window>,
    waiting_on: &mut Option<glib::WeakRef<gtk::Popover>>,
    sender: &relm4::Sender<crate::ui::AppInput>,
) -> bool {
    let Some(open) = window.and_then(|window| open_popover(window.upcast_ref())) else {
        return false;
    };
    if waiting_on.as_ref().and_then(glib::WeakRef::upgrade).as_ref() != Some(&open) {
        let again = sender.clone();
        open.connect_closed(move |_| {
            again.emit(crate::ui::AppInput::SurfaceChanged(
                mailcal_bindings::Surface::Settings,
            ));
        });
        *waiting_on = Some(open.downgrade());
    }
    true
}

/// Sets `edited` when the person changes any field under `root` that waits for a button to send
/// it. Connected after the page is built, so the text a page is drawn with does not count.
pub(super) fn watch_edits(root: &gtk::Widget, edited: &Rc<Cell<bool>>) {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if crate::ui::controls::unsent(&widget).is_none() {
            if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
                let edited = Rc::clone(edited);
                editable.connect_changed(move |_| edited.set(true));
            }
            watch_edits(&widget, edited);
        }
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
