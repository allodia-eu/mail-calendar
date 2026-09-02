//! What the composer's header must draw, and what it must keep behind the chevron.
//!
//! The GTK halves are called from the crate's single `gtk::init` test (see
//! [`super::super::mailbox::tests`]); `reveals_cc_bcc` is a plain unit test.

use adw::prelude::*;

use super::{recipient_rows, reveals_cc_bcc};
use crate::{
    l10n,
    ui::{
        composer_model::{ComposeKind, ComposeRequest},
        recipients::RecipientField,
    },
};

#[test]
fn a_pre_filled_cc_or_bcc_opens_the_collapsed_row() {
    // Cc/Bcc open collapsed, so a pre-filled one has to force them open: a recipient the sender
    // cannot see is one they cannot remove (docs/composer-security.md, Gate 12).
    assert!(reveals_cc_bcc("carol@example.test", ""));
    assert!(reveals_cc_bcc("", "snoop@evil.test"));
    assert!(reveals_cc_bcc("carol@example.test", "snoop@evil.test"));
    assert!(!reveals_cc_bcc("", ""));
    // Whitespace is not an address.
    assert!(!reveals_cc_bcc("  ", " "));
}

#[test]
fn the_caret_opens_in_the_body_only_for_a_composer_that_is_already_addressed() {
    // One predicate, because exactly one of To and the body may take the caret and two flags can
    // disagree (docs/contacts.md §4).
    let new_message = |to: &str| ComposeRequest {
        initial_to: to.to_owned(),
        ..request("")
    };
    assert!(!new_message("").opens_in_body());
    assert!(
        !new_message("  ").opens_in_body(),
        "whitespace is not an address"
    );
    // A mail link, or an assistant's draft: a new message that arrived addressed.
    assert!(new_message("bob@example.test").opens_in_body());
    for kind in [
        ComposeKind::Reply,
        ComposeKind::ReplyAll,
        ComposeKind::Forward,
    ] {
        assert!(
            ComposeRequest {
                kind,
                ..request("")
            }
            .opens_in_body()
        );
    }
}

/// A new message opens on From, To, Subject, and the chevron is the only way to the other two.
pub(crate) fn a_new_message_keeps_cc_and_bcc_behind_the_chevron() {
    let form = gtk::Grid::new();
    let rows = recipient_rows(&form, &request(""), None);
    assert!(row_visible(&rows.to), "To is always on screen");
    assert!(!row_visible(&rows.cc), "Cc opened revealed");
    assert!(!row_visible(&rows.bcc), "Bcc opened revealed");
    assert!(
        !visible_captions(&form).contains(&l10n::compose_cc().to_owned()),
        "the Cc caption is on screen with no field under it"
    );

    let toggle = chevron(&form).expect("the To row draws a Cc/Bcc chevron");
    assert!(!toggle.is_active());
    toggle.set_active(true);
    assert!(row_visible(&rows.cc), "the chevron revealed nothing");
    assert!(row_visible(&rows.bcc), "the chevron revealed nothing");
    assert!(visible_captions(&form).contains(&l10n::compose_bcc().to_owned()));
    toggle.set_active(false);
    assert!(!row_visible(&rows.cc), "the reveal is one-way");
}

/// A reply-all fills Cc, and the row has to open with it: a recipient the sender cannot see is one
/// they cannot remove.
pub(crate) fn a_reply_all_opens_with_its_cc_on_screen() {
    let form = gtk::Grid::new();
    let rows = recipient_rows(&form, &request("carol@example.test"), None);
    assert!(row_visible(&rows.cc));
    assert!(row_visible(&rows.bcc));
    assert!(
        chevron(&form)
            .expect("the To row draws a Cc/Bcc chevron")
            .is_active(),
        "the row is open while the chevron still points down"
    );
    assert!(
        visible_captions(&form).contains(&l10n::compose_cc().to_owned()),
        "the Cc addresses are on screen without their caption"
    );
}

/// A mail link may name Bcc, so the composer must expose that recipient before anything can send.
pub(crate) fn a_mail_link_opens_with_its_bcc_on_screen() {
    let form = gtk::Grid::new();
    let request = ComposeRequest {
        initial_bcc: "snoop@example.test".to_owned(),
        ..request("")
    };
    let rows = recipient_rows(&form, &request, None);
    assert!(row_visible(&rows.cc));
    assert!(row_visible(&rows.bcc));
    assert_eq!(rows.bcc.text(), "snoop@example.test, ");
}

fn request(initial_cc: &str) -> ComposeRequest {
    ComposeRequest {
        kind: ComposeKind::New,
        account: None,
        key: None,
        initial_to: String::new(),
        initial_cc: initial_cc.to_owned(),
        initial_bcc: String::new(),
        subject: String::new(),
        initial_body: None,
        quote: None,
        initial_from: None,
        seeds_signature: true,
        files: Vec::new(),
    }
}

/// Whether a field's row is on screen, read off the box the grid actually holds rather than off
/// the flag that set it: a reveal that never reached `set_visible` would leave a state assertion
/// green with the fields still drawn.
fn row_visible(field: &RecipientField) -> bool {
    field
        .widget()
        .parent()
        .expect("a recipient field is attached to its row box")
        .is_visible()
}

/// The chevron, found by walking the form rather than by holding a reference to it; the test asks
/// the question the user does: is there one on screen.
fn chevron(form: &gtk::Grid) -> Option<gtk::ToggleButton> {
    walk(form.upcast_ref())
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::ToggleButton>().ok())
}

/// Every caption the form actually draws. `is_visible` walks the ancestors, so a caption inside a
/// hidden row answers `false` even though its own flag was never touched.
fn visible_captions(form: &gtk::Grid) -> Vec<String> {
    walk(form.upcast_ref())
        .into_iter()
        .filter(WidgetExt::is_visible)
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .map(|label| label.text().to_string())
        .collect()
}

fn walk(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.push(widget.clone());
        found.extend(walk(&widget));
        child = widget.next_sibling();
    }
    found
}
