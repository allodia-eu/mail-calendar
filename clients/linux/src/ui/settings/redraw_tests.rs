use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{AccountEntry, AccountKind, AccountsSnapshot};

use super::{Signalled, keep_offset, offset, signalled, typing, watch_edits};

fn snapshot(addresses: &[&str]) -> AccountsSnapshot {
    AccountsSnapshot {
        accounts: addresses
            .iter()
            .map(|address| AccountEntry {
                id: format!("{address}@dav"),
                address: (*address).to_owned(),
                kind: AccountKind::Dav,
                uses: Vec::new(),
                links: mailcal_bindings::AccountLinksView::default(),
                linked_from: Vec::new(),
                link_candidates: mailcal_bindings::LinkCandidates::default(),
                endpoints: None,
            })
            .collect(),
    }
}

#[test]
fn a_signal_redraws_only_what_changed_and_never_under_typing() {
    let drawn = snapshot(&["alice@cloud.example"]);
    let mut suggested = drawn.clone();
    suggested.accounts[0].link_candidates.suggested = vec!["bob@dav".to_owned()];

    assert_eq!(signalled(false, None, || drawn.clone()), Signalled::Draw);
    assert_eq!(
        signalled(false, Some(&drawn), || drawn.clone()),
        Signalled::Skip,
        "a signal about another page's setting leaves this one alone"
    );
    assert_eq!(
        signalled(false, Some(&drawn), || suggested.clone()),
        Signalled::Draw
    );
    assert_eq!(
        signalled(true, Some(&drawn), || panic!(
            "not read while the person is typing"
        )),
        Signalled::Wait
    );
}

/// Called from the crate's single `gtk::init` test.
pub(crate) fn a_redraw_keeps_the_persons_place() {
    a_redrawn_page_opens_where_the_last_one_was_scrolled_to();
    a_second_redraw_in_one_frame_keeps_the_offset_the_first_was_given();
    a_field_being_typed_in_holds_a_signalled_redraw();
}

fn a_redrawn_page_opens_where_the_last_one_was_scrolled_to() {
    let window = gtk::Window::new();
    window.set_default_size(300, 200);
    let scroll = gtk::ScrolledWindow::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_size_request(-1, 1000);
    scroll.set_child(Some(&content));
    window.set_child(Some(&scroll));
    keep_offset(&scroll, 300.0);
    window.present();
    let adjustment = scroll.vadjustment();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while adjustment.upper() < 1000.0 && std::time::Instant::now() < deadline {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert!((adjustment.value() - 300.0).abs() < f64::EPSILON);
    let top = content
        .compute_point(&scroll, &gtk::graphene::Point::new(0.0, 0.0))
        .expect("the page is inside its scrolled window");
    assert!(
        (top.y() + 300.0).abs() < 1.0,
        "the page itself moved, not only the scrollbar: its top is at {}",
        top.y()
    );
    window.close();
}

fn a_second_redraw_in_one_frame_keeps_the_offset_the_first_was_given() {
    let pages = gtk::Stack::new();
    let scroll = gtk::ScrolledWindow::new();
    pages.add_named(&scroll, Some("accounts"));
    keep_offset(&scroll, 300.0);
    assert_eq!(
        offset(&pages),
        Some(300.0),
        "not laid out yet, and still owed"
    );
}

fn a_field_being_typed_in_holds_a_signalled_redraw() {
    let window = gtk::Window::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let entry = gtk::PasswordEntry::new();
    let button = gtk::Button::with_label("Reconnect");
    content.append(&entry);
    content.append(&button);
    window.set_child(Some(&content));
    let edited = Rc::new(Cell::new(false));
    watch_edits(content.upcast_ref(), &edited);
    window.present();
    button.grab_focus();

    assert!(!typing(&window, edited.get()));
    entry.grab_focus();
    assert!(typing(&window, edited.get()), "the caret is in a field");

    entry.set_text("hunter2");
    button.grab_focus();
    assert!(
        typing(&window, edited.get()),
        "what was typed outlives the caret moving to the button that submits it"
    );
    window.close();
}
