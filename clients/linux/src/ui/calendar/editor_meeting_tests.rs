//! The editor's Video meeting row, tested without drawing it: what it offers, what it opens
//! showing, and what a save sends.

use mailcal_bindings::{EditorMeeting, EventIntent, Intent, MeetingChange, MeetingOption};

use super::{MeetingRowOption, MeetingSelection};
use crate::{
    l10n,
    ui::calendar::editor::{
        CalendarChoice, EventDetails, EventEditor, EventForm,
        tests::{choice, editing_with, form},
    },
};

fn teams() -> MeetingOption {
    MeetingOption {
        id: "server:teams".to_owned(),
        name: "Microsoft Teams".to_owned(),
        through: None,
    }
}

fn jitsi() -> MeetingOption {
    MeetingOption {
        id: "jitsi".to_owned(),
        name: "Jitsi Meet".to_owned(),
        through: None,
    }
}

fn calendar(id: &str, meeting_choices: Vec<MeetingOption>) -> CalendarChoice {
    CalendarChoice {
        id: id.to_owned(),
        meeting_choices,
        ..choice()
    }
}

fn editing(
    meeting: Option<EditorMeeting>,
    choices: Vec<MeetingOption>,
    occurrence: &str,
) -> EventEditor {
    let detail = EventDetails {
        editor_meeting: meeting,
        occurrence: occurrence.to_owned(),
        is_recurring: !occurrence.is_empty(),
        ..editing_with(occurrence, None).editing.expect("an edit")
    };
    EventEditor::edit(detail, vec![calendar("calendar-a", choices)])
}

fn with_meeting(meeting: MeetingSelection) -> EventForm {
    EventForm {
        meeting,
        ..form(false)
    }
}

fn labels(options: &[MeetingRowOption]) -> Vec<String> {
    options.iter().map(MeetingRowOption::label).collect()
}

fn sent_meeting(
    editor: &EventEditor,
    form: &EventForm,
    this_occurrence_only: bool,
) -> Option<MeetingChange> {
    match editor
        .intent(form, this_occurrence_only)
        .expect("the form is valid")
    {
        Intent::Events {
            intent: EventIntent::Create { meeting, .. } | EventIntent::Update { meeting, .. },
        } => meeting,
        _ => panic!("expected an event write"),
    }
}

fn set_choice(change: Option<MeetingChange>) -> String {
    match change {
        Some(MeetingChange::Set { choice, words }) => {
            assert_eq!(words.meeting, l10n::event_video_meeting());
            assert_eq!(words.join, l10n::event_join());
            assert_eq!(words.meeting_id, l10n::event_meeting_id());
            assert_eq!(words.passcode, l10n::event_meeting_passcode());
            assert_eq!(words.dial_in, l10n::event_meeting_dial_in());
            assert_eq!(words.dial_in_code, l10n::event_meeting_conference_id());
            choice
        }
        other => panic!("expected a meeting to be set, got {other:?}"),
    }
}

#[test]
fn a_new_event_starts_with_no_meeting_and_offers_the_calendars_choices() {
    let editor = EventEditor::create(
        vec![calendar("calendar-a", vec![teams(), jitsi()])],
        String::new(),
    );
    assert!(editor.shows_meeting_row(0));
    assert_eq!(editor.opened_meeting(), MeetingSelection::None);
    assert_eq!(
        editor
            .meeting_options(0)
            .into_iter()
            .map(|option| option.selection)
            .collect::<Vec<_>>(),
        [
            MeetingSelection::None,
            MeetingSelection::Choice("server:teams".to_owned()),
            MeetingSelection::Choice("jitsi".to_owned()),
        ]
    );
    assert_eq!(
        labels(&editor.meeting_options(0)),
        [l10n::event_meeting_none(), "Microsoft Teams", "Jitsi Meet"]
    );
    assert_eq!(sent_meeting(&editor, &form(false), false), None);
    assert_eq!(
        set_choice(sent_meeting(
            &editor,
            &with_meeting(MeetingSelection::Choice("jitsi".to_owned())),
            false
        )),
        "jitsi"
    );
}

#[test]
fn a_meeting_made_through_an_account_is_named_with_the_account() {
    let talk = MeetingOption {
        id: "account:nextcloud-talk:alice@dav:cloud.example.org".to_owned(),
        name: "Nextcloud Talk".to_owned(),
        through: Some("alice@cloud.example.org".to_owned()),
    };
    let editor = EventEditor::create(
        vec![calendar("calendar-a", vec![teams(), talk, jitsi()])],
        String::new(),
    );
    assert_eq!(
        labels(&editor.meeting_options(0)),
        [
            l10n::event_meeting_none().to_owned(),
            "Microsoft Teams".to_owned(),
            l10n::event_meeting_through("Nextcloud Talk", "alice@cloud.example.org"),
            "Jitsi Meet".to_owned(),
        ]
    );
}

#[test]
fn a_choice_the_event_does_not_have_yet_says_the_link_comes_at_the_save() {
    let editor = EventEditor::create(
        vec![calendar("calendar-a", vec![teams(), jitsi()])],
        String::new(),
    );
    let teams_pick = MeetingSelection::Choice("server:teams".to_owned());
    assert!(!editor.announces_meeting(0, &MeetingSelection::None));
    assert!(editor.announces_meeting(0, &teams_pick));

    let opened = editing(
        Some(EditorMeeting {
            name: Some("Microsoft Teams".to_owned()),
            choice: Some("server:teams".to_owned()),
        }),
        vec![teams(), jitsi()],
        "",
    );
    assert!(
        !opened.announces_meeting(0, &teams_pick),
        "the event already has this meeting"
    );
    assert!(opened.announces_meeting(0, &MeetingSelection::Choice("jitsi".to_owned())));
    assert!(!opened.announces_meeting(0, &MeetingSelection::None));

    let elsewhere = editing(
        Some(EditorMeeting {
            name: Some("Zoom".to_owned()),
            choice: None,
        }),
        vec![jitsi()],
        "",
    );
    assert!(!elsewhere.announces_meeting(0, &MeetingSelection::Current));
}

#[test]
fn a_calendar_with_no_choices_draws_no_row() {
    let editor = EventEditor::create(vec![calendar("calendar-a", Vec::new())], String::new());
    assert!(!editor.shows_meeting_row(0));
}

#[test]
fn a_choice_the_new_calendar_lacks_goes_back_to_none() {
    let editor = EventEditor::create(
        vec![
            calendar("work", vec![teams(), jitsi()]),
            calendar("home", vec![jitsi()]),
        ],
        String::new(),
    );
    let teams_pick = MeetingSelection::Choice("server:teams".to_owned());
    assert!(editor.shows_meeting_row(1));
    assert_eq!(
        editor.effective_meeting(1, &teams_pick),
        MeetingSelection::None
    );
    assert_eq!(editor.effective_meeting(0, &teams_pick), teams_pick);
    let moved = EventForm {
        calendar_index: 1,
        ..with_meeting(teams_pick)
    };
    assert_eq!(sent_meeting(&editor, &moved, false), None);
}

#[test]
fn an_edit_opens_on_the_meeting_and_leaves_it_alone_until_the_row_changes() {
    let editor = editing(
        Some(EditorMeeting {
            name: Some("Microsoft Teams".to_owned()),
            choice: Some("server:teams".to_owned()),
        }),
        vec![teams(), jitsi()],
        "",
    );
    let opened = editor.opened_meeting();
    assert_eq!(opened, MeetingSelection::Choice("server:teams".to_owned()));
    assert_eq!(sent_meeting(&editor, &with_meeting(opened), false), None);
    assert_eq!(
        set_choice(sent_meeting(
            &editor,
            &with_meeting(MeetingSelection::Choice("jitsi".to_owned())),
            false
        )),
        "jitsi"
    );
    assert_eq!(
        sent_meeting(&editor, &with_meeting(MeetingSelection::None), false),
        Some(MeetingChange::Remove)
    );
}

#[test]
fn a_meeting_belongs_to_the_series_and_settles_which_occurrences_were_meant() {
    let editor = editing(
        Some(EditorMeeting {
            name: Some("Microsoft Teams".to_owned()),
            choice: Some("server:teams".to_owned()),
        }),
        vec![teams(), jitsi()],
        "2026-07-21T09:00:00",
    );
    let unchanged = with_meeting(editor.opened_meeting());
    assert!(editor.save_asks_about_the_series(&unchanged));
    let removed = with_meeting(MeetingSelection::None);
    assert!(!editor.save_asks_about_the_series(&removed));
    assert_eq!(sent_meeting(&editor, &removed, true), None);
    assert_eq!(
        sent_meeting(&editor, &removed, false),
        Some(MeetingChange::Remove)
    );
}

#[test]
fn a_meeting_no_choice_makes_is_shown_by_name_and_can_only_be_taken_off() {
    let editor = editing(
        Some(EditorMeeting {
            name: Some("Zoom".to_owned()),
            choice: None,
        }),
        vec![jitsi()],
        "",
    );
    assert_eq!(editor.opened_meeting(), MeetingSelection::Current);
    assert_eq!(
        labels(&editor.meeting_options(0)),
        [l10n::event_meeting_none(), "Zoom", "Jitsi Meet"]
    );
    assert_eq!(
        sent_meeting(&editor, &with_meeting(MeetingSelection::Current), false),
        None
    );
    assert_eq!(
        sent_meeting(&editor, &with_meeting(MeetingSelection::None), false),
        Some(MeetingChange::Remove)
    );
}

#[test]
fn a_meeting_on_a_calendar_that_offers_none_can_still_be_taken_off() {
    let editor = editing(
        Some(EditorMeeting {
            name: None,
            choice: Some("jitsi".to_owned()),
        }),
        Vec::new(),
        "",
    );
    assert!(editor.shows_meeting_row(0));
    assert_eq!(
        labels(&editor.meeting_options(0)),
        [l10n::event_meeting_none(), l10n::event_video_meeting()]
    );
    let opened = editor.opened_meeting();
    assert_eq!(editor.effective_meeting(0, &opened), opened);
    assert_eq!(sent_meeting(&editor, &with_meeting(opened), false), None);
}

#[test]
fn an_edit_reads_its_own_calendar_whatever_the_picker_holds() {
    let detail = editing_with("", None).editing.expect("an edit");
    let editor = EventEditor::edit(
        detail,
        vec![
            calendar("other", Vec::new()),
            calendar("calendar-a", vec![jitsi()]),
        ],
    );
    assert!(editor.shows_meeting_row(0));
    assert_eq!(labels(&editor.meeting_options(0)).len(), 2);
}
