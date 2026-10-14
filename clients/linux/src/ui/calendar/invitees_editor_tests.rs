//! The invitee list: what it says about an address it will not add, and that it adds, refuses and
//! hides as the editor's state says.

use adw::prelude::*;
use mailcal_bindings::{InviteeCheck, MeetingInviteeRole};

use super::{append_invitee_section, keeps_its_last_invitee, refusal};
use crate::{
    l10n,
    ui::calendar::editor::{CalendarChoice, EventEditor},
};

#[test]
fn each_refusal_says_why_and_a_valid_address_says_nothing() {
    assert_eq!(
        refusal(&InviteeCheck::Valid {
            address: "grace@example.org".to_owned()
        }),
        None
    );
    assert_eq!(
        refusal(&InviteeCheck::NotAnAddress),
        Some(l10n::event_invitee_not_an_address())
    );
    assert_eq!(
        refusal(&InviteeCheck::AlreadyInvited),
        Some(l10n::event_invitee_already_invited())
    );
    assert_eq!(
        refusal(&InviteeCheck::Organiser),
        Some(l10n::event_invitee_organiser())
    );
}

#[test]
fn an_existing_meeting_keeps_its_last_invitee_and_a_create_does_not_have_to() {
    let row = |email: &str, role, is_organizer| mailcal_bindings::EventAttendee {
        name: String::new(),
        email: email.to_owned(),
        is_organizer,
        role,
        response: mailcal_bindings::ResponseStatus::NeedsAction,
    };
    let meeting = [
        row("me@example.org", None, true),
        row(
            "grace@example.org",
            Some(MeetingInviteeRole::Required),
            false,
        ),
    ];
    assert!(keeps_its_last_invitee(&meeting, 1));
    assert!(!keeps_its_last_invitee(&meeting, 2));
    assert!(
        !keeps_its_last_invitee(&[], 1),
        "a create may become an appointment"
    );
    let chaired = [
        meeting[0].clone(),
        meeting[1].clone(),
        row("chair@example.org", None, false),
    ];
    assert!(
        !keeps_its_last_invitee(&chaired, 1),
        "a chair is still on the meeting"
    );
}

fn choice(id: &str, can_invite: bool) -> CalendarChoice {
    CalendarChoice {
        account: "account-a".to_owned(),
        id: id.to_owned(),
        label: id.to_owned(),
        is_default: id == "work",
        address: "me@example.org".to_owned(),
        can_invite,
    }
}

fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.push(widget.clone());
        found.extend(descendants(&widget));
        child = widget.next_sibling();
    }
    found
}

fn label_showing(root: &gtk::Widget, text: &str) -> Option<gtk::Label> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .find(|label| label.text() == text)
}

/// Visible on screen: its own flag and every ancestor's, up to `root`.
fn shown(widget: &impl IsA<gtk::Widget>, root: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone().upcast::<gtk::Widget>());
    while let Some(widget) = current {
        if !widget.is_visible() {
            return false;
        }
        if &widget == root {
            return true;
        }
        current = widget.parent();
    }
    true
}

pub(crate) fn the_invitee_list_takes_what_is_typed_and_follows_the_calendar() {
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let root = form.clone().upcast::<gtk::Widget>();
    let editor = EventEditor::create(
        vec![choice("work", true), choice("feeds", false)],
        String::new(),
    );
    let calendar = gtk::DropDown::from_strings(&["work", "feeds"]);
    calendar.set_selected(0);
    let (invitees, take_typed) = append_invitee_section(&form, &editor, &calendar, None);
    let entry = descendants(&root)
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::Entry>().ok())
        .expect("the section has an address field");

    entry.set_text("grace@example.org");
    assert!(take_typed(), "a valid address is taken in");
    assert_eq!(invitees.borrow().len(), 1);
    assert_eq!(invitees.borrow()[0].email, "grace@example.org");
    assert_eq!(invitees.borrow()[0].role, MeetingInviteeRole::Required);
    assert_eq!(
        entry.text(),
        "",
        "the field empties once the address is in the list"
    );

    for (typed, said) in [
        ("me@example.org", l10n::event_invitee_organiser()),
        ("GRACE@example.org", l10n::event_invitee_already_invited()),
        ("not an address", l10n::event_invitee_not_an_address()),
    ] {
        entry.set_text(typed);
        assert!(!take_typed(), "{typed} is refused");
        let error = label_showing(&root, said).expect("the refusal is said");
        assert!(shown(&error, &root), "the refusal for {typed} is on screen");
        assert_eq!(
            entry.text(),
            typed,
            "a refused address stays to be corrected"
        );
    }
    assert_eq!(invitees.borrow().len(), 1);

    let optional = descendants(&root)
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::CheckButton>().ok())
        .expect("each invitee has an optional box");
    optional.set_active(true);
    assert_eq!(invitees.borrow()[0].role, MeetingInviteeRole::Optional);
    optional.set_active(false);
    assert_eq!(invitees.borrow()[0].role, MeetingInviteeRole::Required);

    let unavailable =
        label_showing(&root, l10n::event_invitees_unavailable()).expect("the reason is built");
    assert!(!shown(&unavailable, &root));
    calendar.set_selected(1);
    assert!(
        shown(&unavailable, &root),
        "a calendar that cannot invite says why"
    );
    assert!(!shown(&entry, &root), "and offers no field");
    calendar.set_selected(0);
    assert!(shown(&entry, &root));
}
