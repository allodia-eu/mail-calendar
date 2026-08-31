//! The editor's pure decisions: which intent a form produces, and which occurrences a
//! save says it meant.
//!
//! Split out of `editor.rs` to keep that file inside the 500-line cap.

use mailcal_bindings::Intent;
use time::{Date, Month, PrimitiveDateTime, Time};

use super::{CalendarChoice, EventDetails, EventEditor, EventForm};

fn choice() -> CalendarChoice {
    CalendarChoice {
        account: "account-a".to_owned(),
        id: "calendar-a".to_owned(),
        label: "Account · Work".to_owned(),
        is_default: true,
    }
}

fn form(all_day: bool) -> EventForm {
    EventForm {
        title: " Planning ".to_owned(),
        start: if all_day {
            "2026-07-21".to_owned()
        } else {
            "2026-07-21T10:00:00".to_owned()
        },
        end: if all_day {
            "2026-07-21".to_owned()
        } else {
            "2026-07-21T11:00:00".to_owned()
        },
        all_day,
        location: String::new(),
        notes: String::new(),
        calendar_index: 0,
    }
}

#[test]
fn create_uses_the_device_zone_and_exclusive_all_day_end() {
    let now = PrimitiveDateTime::new(
        Date::from_calendar_date(2026, Month::July, 21).unwrap(),
        Time::from_hms(10, 15, 0).unwrap(),
    );
    let editor = EventEditor::create_at(vec![choice()], "Europe/Amsterdam".to_owned(), now);
    assert_eq!(editor.start, "2026-07-21T11:00:00");
    match editor.intent(&form(true), false).unwrap() {
        Intent::CreateEvent {
            title,
            end,
            account,
            calendar,
            timezone,
            ..
        } => {
            assert_eq!(title, "Planning");
            assert_eq!(end, "2026-07-22");
            assert_eq!(account.as_deref(), Some("account-a"));
            assert_eq!(calendar.as_deref(), Some("calendar-a"));
            assert_eq!(timezone, None);
        }
        _ => panic!("expected a create intent"),
    }
}

#[test]
fn dragged_slot_is_not_rounded_again_and_selects_the_default_calendar() {
    let mut first = choice();
    first.id = "first".to_owned();
    first.is_default = false;
    let editor = EventEditor::create_slot(
        vec![first, choice()],
        "Europe/Amsterdam".to_owned(),
        crate::ui::calendar::drag::CreateSlot {
            date: Date::from_calendar_date(2026, Month::July, 22).unwrap(),
            start_minutes: 10 * 60 + 15,
            end_minutes: 11 * 60 + 45,
        },
    );
    assert_eq!(editor.start, "2026-07-22T10:15:00");
    assert_eq!(editor.end, "2026-07-22T11:45:00");
    assert_eq!(editor.selected, 1);
}

#[test]
fn toggling_a_new_timed_form_to_all_day_accepts_its_default_values() {
    let editor = EventEditor::create_at(
        vec![choice()],
        "Europe/Amsterdam".to_owned(),
        PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::July, 21).unwrap(),
            Time::from_hms(10, 15, 0).unwrap(),
        ),
    );
    let mut toggled = form(false);
    toggled.all_day = true;
    assert_eq!(
        EventEditor::values_for_mode(&toggled.start, &toggled.end, true),
        ("2026-07-21".to_owned(), "2026-07-21".to_owned())
    );
    match editor
        .intent(&toggled, false)
        .expect("timed defaults become dates")
    {
        Intent::CreateEvent {
            start,
            end,
            all_day,
            ..
        } => {
            assert_eq!(start, "2026-07-21");
            assert_eq!(end, "2026-07-22");
            assert!(all_day);
        }
        _ => panic!("expected a create intent"),
    }
}

#[test]
fn edit_keeps_wall_clocks_and_blank_optional_fields_clear_properties() {
    let detail = EventDetails {
        account: "account-a".to_owned(),
        key: "event-a".to_owned(),
        calendar: "calendar-a".to_owned(),
        calendar_name: "Work".to_owned(),
        title: "Old".to_owned(),
        all_day: false,
        timezone: "Europe/Amsterdam".to_owned(),
        start: "2026-07-21T09:00:00".to_owned(),
        end: "2026-07-21T10:00:00".to_owned(),
        location: Some("Room".to_owned()),
        notes: None,
        reminder_minutes: None,
        recurrence: None,
        repeat_summary: None,
        is_recurring: false,
        can_write: true,
        occurrence: String::new(),
        attendees: Vec::new(),
    };
    let editor = EventEditor::edit(detail, vec![choice()]);
    match editor.intent(&form(false), false).unwrap() {
        Intent::UpdateEvent {
            start,
            end,
            notes,
            location,
            occurrence,
            ..
        } => {
            assert_eq!(start.as_deref(), Some("2026-07-21T10:00:00"));
            assert_eq!(end.as_deref(), Some("2026-07-21T11:00:00"));
            assert_eq!(notes.as_deref(), Some(""));
            assert_eq!(location.as_deref(), Some(""));
            assert_eq!(occurrence, None);
        }
        _ => panic!("expected an update intent"),
    }
}

/// An editor prefilled from an event the core resolved `occurrence` for.
fn editing(occurrence: &str) -> EventEditor {
    EventEditor::edit(
        EventDetails {
            account: "account-a".to_owned(),
            key: "event-a".to_owned(),
            calendar: "calendar-a".to_owned(),
            calendar_name: "Work".to_owned(),
            title: "Standup".to_owned(),
            all_day: false,
            timezone: "Europe/Amsterdam".to_owned(),
            start: "2026-07-21T09:00:00".to_owned(),
            end: "2026-07-21T10:00:00".to_owned(),
            location: None,
            notes: None,
            reminder_minutes: None,
            recurrence: None,
            repeat_summary: None,
            is_recurring: true,
            can_write: true,
            occurrence: occurrence.to_owned(),
            attendees: Vec::new(),
        },
        vec![choice()],
    )
}

#[test]
fn an_editor_opened_on_one_occurrence_asks_which_ones_the_save_meant() {
    assert!(editing("2026-09-09T09:00:00").asks_about_the_series());
    // An agenda row *is* the series, and a one-off event has no occurrence to name.
    assert!(!editing("").asks_about_the_series());
}

#[test]
fn this_event_sends_the_occurrence_and_all_events_withholds_it() {
    // The whole scope question comes down to this one field, so both answers are asserted:
    // withholding it on *This event* rewrites every occurrence, and sending it on *All
    // events* splits an override instead of moving the series.
    let editor = editing("2026-09-09T09:00:00");
    let occurrence_of =
        |this_occurrence_only| match editor.intent(&form(false), this_occurrence_only).unwrap() {
            Intent::UpdateEvent { occurrence, .. } => occurrence,
            _ => panic!("expected an update intent"),
        };
    assert_eq!(occurrence_of(true).as_deref(), Some("2026-09-09T09:00:00"));
    assert_eq!(occurrence_of(false), None);
}

#[test]
fn an_editor_on_the_series_names_no_occurrence_either_way() {
    // Nothing to name, so even the answer that would send one cannot: an empty token would
    // have the core refuse a write that should have gone through.
    let editor = editing("");
    for this_occurrence_only in [true, false] {
        match editor.intent(&form(false), this_occurrence_only).unwrap() {
            Intent::UpdateEvent { occurrence, .. } => assert_eq!(occurrence, None),
            _ => panic!("expected an update intent"),
        }
    }
}

#[test]
fn editor_rejects_empty_or_backwards_intervals() {
    let editor = EventEditor::create_at(vec![choice()], "UTC".to_owned(), PrimitiveDateTime::MIN);
    let mut invalid = form(false);
    invalid.end = invalid.start.clone();
    assert!(matches!(editor.intent(&invalid, false), Err("range")));
    invalid.title.clear();
    assert!(matches!(editor.intent(&invalid, false), Err("title")));
}
