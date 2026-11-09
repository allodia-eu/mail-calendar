//! The server-change question's copy, and how it is drawn from the core's record.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{AccountServerChange, ControlState, ServerChange};

use super::{body_markup, content, title};
use crate::{l10n, ui::controls::tests::spins};

fn change(calendar: bool, contacts: bool) -> ServerChange {
    ServerChange {
        previous_site: "icloud.com".to_owned(),
        proposed_site: "example.net".to_owned(),
        calendar,
        contacts,
    }
}

/// The text a person reads, once Pango has taken the markup apart; it fails on markup it cannot
/// parse, which a label would show as nothing.
fn read(markup: &str) -> String {
    let (_, text, _) = gtk::pango::parse_markup(markup, '\0').expect("the body is valid markup");
    text.to_string()
}

#[test]
fn the_question_names_what_moved_both_or_the_one_that_did() {
    let address = "alice@example.org";
    for (change, heading, body, unsure) in [
        (
            change(true, false),
            l10n::server_changed_title_calendar(),
            l10n::server_changed_body_calendar(address),
            l10n::server_changed_unsure_calendar(),
        ),
        (
            change(false, true),
            l10n::server_changed_title_contacts(),
            l10n::server_changed_body_contacts(address),
            l10n::server_changed_unsure_contacts(),
        ),
        (
            change(true, true),
            l10n::server_changed_title_both(),
            l10n::server_changed_body_both(address),
            l10n::server_changed_unsure_both(),
        ),
    ] {
        assert_eq!(title(&change), heading);
        let shown = read(&body_markup(&change, address));
        assert_eq!(
            shown,
            format!(
                "{body}\n\n{}\n{}\n\n{}\n\n{unsure}",
                l10n::server_changed_before("icloud.com"),
                l10n::server_changed_now("example.net"),
                l10n::server_changed_advice(),
            ),
            "{change:?}"
        );
    }
}

#[test]
fn only_the_sites_are_bold_and_nothing_the_server_named_is_markup() {
    let mut moved = change(true, false);
    moved.proposed_site = "a&b<i>.example".to_owned();
    let markup = body_markup(&moved, "alice@example.org");

    assert!(markup.contains("<b>icloud.com</b>"));
    assert!(markup.contains("<b>a&amp;b&lt;i&gt;.example</b>"));
    assert_eq!(markup.matches("<b>").count(), 2);
    assert!(read(&markup).contains(&l10n::server_changed_now("a&b<i>.example")));
}

/// The buttons in the order drawn: "Not now", then "Allow".
fn buttons(content: &gtk::Box) -> (gtk::Button, gtk::Button) {
    let row = content.last_child().expect("the buttons");
    let not_now = row
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("Not now");
    let allow = row
        .last_child()
        .and_downcast::<gtk::Button>()
        .expect("Allow");
    (not_now, allow)
}

fn says_failed(content: &gtk::Box) -> bool {
    let mut child = content.first_child();
    while let Some(widget) = child {
        if widget
            .downcast_ref::<gtk::Label>()
            .is_some_and(|label| label.text() == l10n::server_changed_failed())
        {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}

/// Allow sends the change; while it runs, neither button nor Escape answers, and a failed allow
/// is said in the question. Called from the crate's single `gtk::init` test.
pub(crate) fn the_question_holds_while_allow_runs_and_says_a_failure() {
    let dialog = adw::Dialog::new();
    let question = |allow, failed| AccountServerChange {
        change: change(true, false),
        allow,
        failed,
    };
    let pressed = Rc::new(Cell::new(0));
    let heard = pressed.clone();
    let free = ControlState {
        enabled: true,
        running: false,
    };

    let drawn = content(
        &dialog,
        &question(free, false),
        "alice@example.org",
        move || heard.set(heard.get() + 1),
    );
    let (not_now, allow) = buttons(&drawn);
    assert!(not_now.is_sensitive() && allow.is_sensitive() && dialog.can_close());
    assert!(!says_failed(&drawn));
    allow.emit_clicked();
    assert_eq!(pressed.get(), 1);

    let running = ControlState {
        enabled: false,
        running: true,
    };
    let drawn = content(
        &dialog,
        &question(running, false),
        "alice@example.org",
        || {},
    );
    let (not_now, allow) = buttons(&drawn);
    assert!(!not_now.is_sensitive() && !allow.is_sensitive() && spins(&allow));
    assert!(!dialog.can_close(), "closing would not stop the allow");

    let drawn = content(&dialog, &question(free, true), "alice@example.org", || {});
    assert!(says_failed(&drawn) && dialog.can_close());
}
