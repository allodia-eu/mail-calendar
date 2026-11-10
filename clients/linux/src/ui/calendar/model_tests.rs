use mailcal_bindings::{
    CalendarColor, CalendarRow, EventRecurrence, RecurrenceEnd, RecurrenceFrequency,
    SimpleRecurrence, Swatch,
};
use time::{Date, Month};

use super::{CalendarDialog, CalendarMode, CalendarModel};
use crate::ui::calendar::{date::period_title, editor::EventDetails};

#[test]
fn narrow_zoom_keeps_three_day_navigation_contiguous() {
    let monday = Date::from_calendar_date(2026, Month::July, 20).unwrap();
    let mut model = CalendarModel::empty(monday);
    model.week_anchor = monday;
    model.focus = monday + time::Duration::days(2);
    model.mode = CalendarMode::ThreeDay;
    assert_eq!(model.visible_day_range(), (2, 3));
    assert_eq!(model.range_columns(), 7);

    model.focus = monday + time::Duration::days(6);
    assert_eq!(model.visible_day_range(), (6, 3));
    assert_eq!(model.range_columns(), 9);
    model.mode = CalendarMode::Week;
    assert_eq!(model.visible_day_range(), (0, 7));
}

#[test]
fn period_title_names_every_visible_month_and_year() {
    let first = Date::from_calendar_date(2026, Month::June, 29).unwrap();
    let last = first + time::Duration::days(6);
    let mut model = CalendarModel::empty(first);
    model.week_anchor = first;
    model.focus = first;
    model.mode = CalendarMode::Week;
    assert!(model.period_title().contains(&period_title(first, last)));

    let first = Date::from_calendar_date(2026, Month::December, 28).unwrap();
    let last = first + time::Duration::days(6);
    model.week_anchor = first;
    model.focus = first;
    assert!(model.period_title().contains(&period_title(first, last)));
}

#[test]
fn a_calendar_is_named_by_its_accounts_address_even_when_the_account_has_no_mail() {
    let swatch = Swatch {
        background: "#000000".to_owned(),
        text: "#ffffff".to_owned(),
        border: "#000000".to_owned(),
    };
    let today = Date::from_calendar_date(2026, Month::July, 20).unwrap();
    let mut model = CalendarModel::empty(today);
    // An account used only for its calendar is in no mail list: its id is not a name.
    model.page.calendars = vec![CalendarRow {
        account: "dennis@dav:nextcloud.example.org".to_owned(),
        account_address: "info@example.org".to_owned(),
        id: "personal".to_owned(),
        name: "Personal".to_owned(),
        color: CalendarColor {
            hex: "#000000".to_owned(),
            light: swatch.clone(),
            dark: swatch,
        },
        visible: true,
        can_write: true,
        can_invite: true,
        meeting_choices: Vec::new(),
        default_meeting: None,
        is_default: true,
    }];
    model.begin_create();
    let Some(CalendarDialog::Editor(editor)) = model.dialog else {
        panic!("expected the editor");
    };
    assert_eq!(
        editor
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>(),
        ["info@example.org · Personal"]
    );
}

#[test]
fn deleting_from_detail_carries_the_recurring_series_scope() {
    let today = Date::from_calendar_date(2026, Month::July, 20).unwrap();
    let mut model = CalendarModel::empty(today);
    model.dialog = Some(CalendarDialog::Detail(detail(true)));
    model.request_delete_current();
    let Some(CalendarDialog::ConfirmDelete(request)) = model.dialog else {
        panic!("expected delete confirmation");
    };
    assert!(request.is_recurring);
    assert_eq!(request.identity.key, "event-a");
}

#[test]
fn mode_indices_are_stable_for_the_header_menu() {
    assert_eq!(CalendarMode::from_index(0), CalendarMode::Day);
    assert_eq!(CalendarMode::from_index(1), CalendarMode::ThreeDay);
    assert_eq!(CalendarMode::from_index(2), CalendarMode::WorkWeek);
    assert_eq!(CalendarMode::from_index(3), CalendarMode::Week);
    assert_eq!(CalendarMode::from_index(4), CalendarMode::Month);
    assert_eq!(CalendarMode::from_index(5), CalendarMode::Agenda);
}

fn detail(is_recurring: bool) -> EventDetails {
    EventDetails {
        account: "account-a".to_owned(),
        key: "event-a".to_owned(),
        calendar: "calendar-a".to_owned(),
        calendar_name: "Work".to_owned(),
        title: "Planning".to_owned(),
        all_day: false,
        timezone: "Europe/Amsterdam".to_owned(),
        start: "2026-07-21T09:00:00".to_owned(),
        end: "2026-07-21T10:00:00".to_owned(),
        location: None,
        notes: None,
        display_location: None,
        display_notes: None,
        conferences: Vec::new(),
        reminder_minutes: None,
        recurrence: Some(EventRecurrence::Simple {
            rule: SimpleRecurrence {
                frequency: RecurrenceFrequency::Weekly,
                interval: 1,
                days: Vec::new(),
                month_days: Vec::new(),
                months: Vec::new(),
                end: RecurrenceEnd::Never,
            },
        }),
        repeat_summary: None,
        repeat_draft: None,
        is_recurring,
        can_write: true,
        can_edit_invitees: false,
        my_answer: None,
        occurrence: String::new(),
        attendees: Vec::new(),
        editor_meeting: None,
    }
}
