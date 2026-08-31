//! The attendee row's two rules: what its second line says, and that neither line is ever parsed
//! as markup.

use adw::prelude::PreferencesRowExt;
use gtk::prelude::{Cast, WidgetExt};
use mailcal_bindings::{EventAttendee, ResponseStatus};

use super::{attendee_row, attendee_subtitle};
use crate::l10n;

fn attendee(name: &str, email: &str, is_organizer: bool) -> EventAttendee {
    EventAttendee {
        name: name.to_owned(),
        email: email.to_owned(),
        response: ResponseStatus::Accepted,
        is_organizer,
    }
}

/// The row is already showing the address as its title, so repeating it underneath says nothing.
#[test]
fn an_unnamed_attendee_is_not_shown_their_own_address_twice() {
    assert_eq!(
        attendee_subtitle(&attendee("", "bob@test.local", false)),
        ""
    );
}

#[test]
fn a_named_attendee_gets_their_address_underneath() {
    assert_eq!(
        attendee_subtitle(&attendee("Bob", "bob@test.local", false)),
        "bob@test.local"
    );
}

#[test]
fn whoever_called_the_meeting_is_marked_either_way() {
    assert_eq!(
        attendee_subtitle(&attendee("Bob", "bob@test.local", true)),
        format!("bob@test.local · {}", l10n::event_attendee_organizer())
    );
    assert_eq!(
        attendee_subtitle(&attendee("", "bob@test.local", true)),
        l10n::event_attendee_organizer()
    );
}

/// A display name is attacker-controlled. The ampersand goes in **both** halves: the property
/// builder applied `use-markup` after the title but before the subtitle, so a name-only fixture
/// stayed silent while a real address in the second line warned.
pub(crate) fn attendee_rows_never_parse_a_name_as_markup() {
    let (row, records) = crate::ui::mailbox::tests::glib_records(|| {
        attendee_row(&attendee("Research & Development", "r&d@test.local", false))
    });
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "an attendee row must not parse a name as markup: {records:?}"
    );
    assert!(!row.uses_markup());

    // Assert on the rendered label, not the property: `ActionRow::title()` hands back the string
    // it was given whatever the label did, which is a green assertion for a blank row.
    let rendered = labels(row.upcast_ref::<gtk::Widget>());
    assert!(
        rendered.iter().any(|text| text == "Research & Development"),
        "the name must render as itself: {rendered:?}"
    );
    assert!(
        rendered.iter().any(|text| text == "r&d@test.local"),
        "the address must render as itself: {rendered:?}"
    );
}

fn labels(root: &gtk::Widget) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.text().to_string());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(labels(&node));
        child = node.next_sibling();
    }
    found
}
