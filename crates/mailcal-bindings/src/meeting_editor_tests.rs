use super::*;
use crate::{MeetingInviteeRole, ResponseStatus};

fn attendee(email: &str, role: Option<MeetingInviteeRole>, is_organizer: bool) -> EventAttendee {
    EventAttendee {
        name: String::new(),
        email: email.to_owned(),
        is_organizer,
        role,
        response: ResponseStatus::NeedsAction,
    }
}

fn invitee(email: &str, role: MeetingInviteeRole) -> MeetingInvitee {
    MeetingInvitee {
        name: None,
        email: email.to_owned(),
        role,
    }
}

const REQUIRED: MeetingInviteeRole = MeetingInviteeRole::Required;
const OPTIONAL: MeetingInviteeRole = MeetingInviteeRole::Optional;

fn meeting() -> Vec<EventAttendee> {
    vec![
        attendee("me@example.org", None, true),
        attendee("grace@example.org", Some(REQUIRED), false),
        attendee("ada@example.org", Some(OPTIONAL), false),
        attendee("chair@example.org", None, false),
    ]
}

#[test]
fn a_typed_address_is_checked_and_canonicalised() {
    let check = |typed: &str| {
        check_meeting_invitee(
            typed.to_owned(),
            vec!["grace@example.org".to_owned()],
            "me@example.org".to_owned(),
        )
    };
    assert_eq!(
        check("  Linus@Example.ORG "),
        InviteeCheck::Valid {
            address: "Linus@example.org".to_owned()
        }
    );
    assert_eq!(check("not an address"), InviteeCheck::NotAnAddress);
    assert_eq!(check(""), InviteeCheck::NotAnAddress);
    assert_eq!(check("GRACE@example.org"), InviteeCheck::AlreadyInvited);
    assert_eq!(check("me@EXAMPLE.org"), InviteeCheck::Organiser);
}

#[test]
fn an_unchanged_list_changes_nothing() {
    let invitees = vec![
        invitee("grace@example.org", REQUIRED),
        invitee("Ada@Example.org", OPTIONAL),
    ];
    assert_eq!(meeting_invitee_patch(meeting(), invitees), None);
}

#[test]
fn an_added_invitee_and_a_changed_role_are_upserts() {
    let invitees = vec![
        invitee("grace@example.org", OPTIONAL),
        invitee("ada@example.org", OPTIONAL),
        invitee("linus@example.org", REQUIRED),
    ];
    let patch = meeting_invitee_patch(meeting(), invitees).expect("the list changed");
    assert_eq!(
        patch.upsert,
        vec![
            invitee("grace@example.org", OPTIONAL),
            invitee("linus@example.org", REQUIRED),
        ]
    );
    assert!(patch.remove.is_empty());
}

#[test]
fn an_invitee_left_off_the_list_is_removed() {
    let patch = meeting_invitee_patch(meeting(), vec![invitee("grace@example.org", REQUIRED)])
        .expect("the list changed");
    assert!(patch.upsert.is_empty());
    assert_eq!(patch.remove, vec!["ada@example.org".to_owned()]);
}

#[test]
fn a_row_the_organiser_cannot_edit_is_never_removed_or_changed() {
    // Neither the organiser nor the chair is in the edited list, and the chair appears once more
    // with a role it does not have: both are rows no control could have changed.
    let invitees = vec![
        invitee("grace@example.org", REQUIRED),
        invitee("ada@example.org", OPTIONAL),
        invitee("chair@example.org", OPTIONAL),
    ];
    assert_eq!(meeting_invitee_patch(meeting(), invitees), None);
}
