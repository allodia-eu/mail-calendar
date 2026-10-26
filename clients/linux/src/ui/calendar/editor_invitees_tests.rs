//! The editor's invitee decisions: where the list is offered, what a create carries, and what an
//! organiser's edit sends.

use mailcal_bindings::{
    EventAttendee, EventIntent, Intent, MeetingInvitee, MeetingInviteePatch, MeetingInviteeRole,
    ResponseStatus,
};

use super::{
    CalendarChoice, EventEditor, EventForm,
    tests::{choice, editing_with, form},
};

fn inviting() -> CalendarChoice {
    CalendarChoice {
        can_invite: true,
        ..choice()
    }
}

fn invitee(email: &str, role: MeetingInviteeRole) -> MeetingInvitee {
    MeetingInvitee {
        name: None,
        email: email.to_owned(),
        role,
    }
}

fn attendee(email: &str, role: Option<MeetingInviteeRole>, is_organizer: bool) -> EventAttendee {
    EventAttendee {
        name: String::new(),
        email: email.to_owned(),
        is_organizer,
        role,
        response: ResponseStatus::NeedsAction,
    }
}

fn with_invitees(invitees: Vec<MeetingInvitee>) -> EventForm {
    EventForm {
        invitees,
        ..form(false)
    }
}

/// A meeting this account organises, opened on `occurrence` (empty for the series).
fn organising(occurrence: &str) -> EventEditor {
    let mut editor = editing_with(occurrence, None);
    let detail = editor.editing.as_mut().expect("an edit");
    detail.can_edit_invitees = true;
    detail.attendees = vec![
        attendee("me@example.org", None, true),
        attendee(
            "grace@example.org",
            Some(MeetingInviteeRole::Required),
            false,
        ),
        attendee("chair@example.org", None, false),
    ];
    editor
}

fn created_invitees(editor: &EventEditor, form: &EventForm) -> Option<Vec<MeetingInvitee>> {
    match editor.intent(form, false).expect("a valid form") {
        Intent::Events {
            intent: EventIntent::Create { invitees, .. },
        } => invitees,
        _ => panic!("expected a create"),
    }
}

fn updated_invitees(
    editor: &EventEditor,
    form: &EventForm,
    this_occurrence_only: bool,
) -> Option<MeetingInviteePatch> {
    match editor
        .intent(form, this_occurrence_only)
        .expect("a valid form")
    {
        Intent::Events {
            intent: EventIntent::Update { invitees, .. },
        } => invitees,
        _ => panic!("expected an update"),
    }
}

#[test]
fn a_create_invites_only_on_a_calendar_whose_server_sends_invitations() {
    let list = vec![invitee("grace@example.org", MeetingInviteeRole::Optional)];
    let on = EventEditor::create(vec![inviting()], String::new());
    assert!(on.can_invite(0));
    assert_eq!(
        created_invitees(&on, &with_invitees(list.clone())),
        Some(list.clone())
    );

    let off = EventEditor::create(vec![choice()], String::new());
    assert!(!off.can_invite(0));
    assert_eq!(created_invitees(&off, &with_invitees(list)), None);
}

#[test]
fn a_create_with_nobody_on_the_list_is_an_appointment() {
    let editor = EventEditor::create(vec![inviting()], String::new());
    assert_eq!(created_invitees(&editor, &with_invitees(Vec::new())), None);
}

#[test]
fn the_organiser_of_a_create_is_the_chosen_calendars_account() {
    let editor = EventEditor::create(vec![inviting()], String::new());
    assert_eq!(editor.organiser(0), "me@example.org");
    assert_eq!(organising("").organiser(0), "me@example.org");
}

#[test]
fn an_organiser_opens_the_list_with_the_rows_they_may_edit() {
    assert_eq!(
        organising("").initial_invitees(),
        vec![invitee("grace@example.org", MeetingInviteeRole::Required)]
    );
    assert_eq!(editing_with("", None).initial_invitees(), []);
}

#[test]
fn an_organiser_sends_what_changed_and_nothing_when_nothing_did() {
    let editor = organising("");
    let unchanged = with_invitees(editor.initial_invitees());
    assert_eq!(updated_invitees(&editor, &unchanged, false), None);

    let changed = with_invitees(vec![invitee(
        "ada@example.org",
        MeetingInviteeRole::Optional,
    )]);
    assert_eq!(
        updated_invitees(&editor, &changed, false),
        Some(MeetingInviteePatch {
            upsert: vec![invitee("ada@example.org", MeetingInviteeRole::Optional)],
            remove: vec!["grace@example.org".to_owned()],
        })
    );
}

#[test]
fn somebody_elses_meeting_sends_no_roster() {
    let editor = editing_with("", None);
    let changed = with_invitees(vec![invitee(
        "ada@example.org",
        MeetingInviteeRole::Optional,
    )]);
    assert!(!editor.can_invite(0));
    assert_eq!(updated_invitees(&editor, &changed, false), None);
}

#[test]
fn a_changed_roster_settles_the_scope_question_and_never_travels_with_one_occurrence() {
    let editor = organising("2026-09-09T09:00:00");
    let unchanged = with_invitees(editor.initial_invitees());
    assert!(editor.save_asks_about_the_series(&unchanged));

    let changed = with_invitees(Vec::new());
    assert!(!editor.save_asks_about_the_series(&changed));
    assert!(updated_invitees(&editor, &changed, false).is_some());
    assert_eq!(updated_invitees(&editor, &changed, true), None);
}
