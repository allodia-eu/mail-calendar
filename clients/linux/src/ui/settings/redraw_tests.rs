use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{AccountEntry, AccountKind, AccountsSnapshot};

use super::{Signalled, keep_offset, offset, signalled, typing, watch_edits};

fn snapshot(addresses: &[&str]) -> AccountsSnapshot {
    AccountsSnapshot {
        meeting_accounts: Vec::new(),
        accounts: addresses
            .iter()
            .map(|address| AccountEntry {
                id: format!("{address}@dav"),
                address: (*address).to_owned(),
                kind: AccountKind::Dav,
                sign_in: mailcal_bindings::SignInMethod::Password,
                uses: Vec::new(),
                links: mailcal_bindings::AccountLinksView::default(),
                linked_from: Vec::new(),
                link_candidates: mailcal_bindings::LinkCandidates::default(),
                endpoints: None,
                separate_server: None,
                distinction: None,
                mail_sync: None,
                nextcloud: false,
            })
            .collect(),
    }
}

#[test]
fn a_signal_redraws_only_what_changed_and_never_under_typing() {
    let drawn = snapshot(&["alice@cloud.example"]);
    let mut suggested = drawn.clone();
    suggested.accounts[0].link_candidates.suggested = vec!["bob@dav".to_owned()];

    assert_eq!(
        signalled(false, None, || drawn.clone()),
        Signalled::Draw(drawn.clone())
    );
    assert_eq!(
        signalled(false, Some(&drawn), || drawn.clone()),
        Signalled::Skip,
        "a signal about another page's setting leaves this one alone"
    );
    assert_eq!(
        signalled(false, Some(&drawn), || suggested.clone()),
        Signalled::Draw(suggested.clone())
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
    a_field_that_sends_itself_holds_a_redraw_only_until_it_has();
    a_form_holds_a_redraw_until_its_button_sends_it();
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

fn a_field_that_sends_itself_holds_a_redraw_only_until_it_has() {
    let window = gtk::Window::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let sent = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sending = Rc::clone(&sent);
    let free = mailcal_bindings::ControlState {
        enabled: true,
        running: false,
    };
    let row = crate::ui::controls::text_row("Your name", "", "Alice", free, move |name| {
        sending.borrow_mut().push(name);
    });
    let button = gtk::Button::with_label("Other");
    content.append(&row);
    content.append(&button);
    window.set_child(Some(&content));
    let edited = Rc::new(Cell::new(false));
    watch_edits(content.upcast_ref(), &edited);
    window.present();
    let entry = find_entry(row.upcast_ref()).expect("the row holds its field");
    entry.grab_focus();
    assert!(
        !typing(&window, edited.get()),
        "the caret alone, in a field holding what was sent, holds nothing"
    );

    entry.set_text("Alice Example");
    assert!(
        typing(&window, edited.get()),
        "what is typed is not sent yet"
    );
    entry.emit_activate();
    assert!(
        !typing(&window, edited.get()),
        "once sent, the page can be redrawn with the caret still there"
    );
    button.grab_focus();
    assert!(
        !edited.get(),
        "a field that sends itself is no edit waiting for a button"
    );
    assert_eq!(
        *sent.borrow(),
        vec!["Alice Example".to_owned()],
        "Return and then the focus leaving send it once"
    );
    window.close();
}

fn find_entry(root: &gtk::Widget) -> Option<gtk::Entry> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
            return Some(entry);
        }
        if let Some(entry) = find_entry(&widget) {
            return Some(entry);
        }
        child = widget.next_sibling();
    }
    None
}

fn a_form_holds_a_redraw_until_its_button_sends_it() {
    let window = gtk::Window::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let host = gtk::Entry::new();
    form.append(&host);
    let elsewhere = gtk::Button::with_label("Other");
    content.append(&form);
    content.append(&elsewhere);
    window.set_child(Some(&content));
    crate::ui::controls::form_unsent(&form, false);
    let edited = Rc::new(Cell::new(false));
    watch_edits(content.upcast_ref(), &edited);
    window.present();

    host.grab_focus();
    assert!(
        !typing(&window, edited.get()),
        "the caret in a form holding what was sent holds nothing"
    );
    host.set_text("imap.example.org");
    crate::ui::controls::form_unsent(&form, true);
    elsewhere.grab_focus();
    assert!(
        typing(&window, edited.get()),
        "an unsent form holds a redraw wherever the caret went"
    );
    crate::ui::controls::form_unsent(&form, false);
    assert!(
        !typing(&window, edited.get()),
        "once its button sent it, the page can be redrawn"
    );
    assert!(
        !edited.get(),
        "a form that sends itself is no edit the window waits on"
    );
    window.close();
}
