//! What the calendar pane asks the core for, and what it says about a day
//! (`docs/calendar-pane.md`, rules 3, 4 and 11).

use std::collections::HashMap;

use gtk::gdk::Key;
use mailcal_bindings::{
    CalendarColor, CalendarGroup, CalendarLayout, CalendarRow, MiniDay, NoticeSeverity, ShownSpan,
    Swatch,
};
use time::{Date, Month};

use super::{CalendarSide, Shown};
use crate::{
    l10n,
    ui::{
        calendar::{
            model::{CalendarMode, CalendarModel},
            side_pane_list::keys,
            side_pane_month::{arrow_target, band_class, spoken},
        },
        notice_rules::AccountMark,
    },
};

fn day(value: u8) -> Date {
    Date::from_calendar_date(2026, Month::October, value).unwrap()
}

/// A grid on Wednesday 28 October 2026, in a week that starts on Monday the 26th.
fn calendar(mode: CalendarMode) -> CalendarModel {
    let mut calendar = CalendarModel::empty(day(28));
    calendar.mode = mode;
    calendar.week_anchor = day(26);
    calendar
}

#[test]
fn a_week_names_its_first_day_and_seven_columns() {
    assert_eq!(
        Shown::of(&calendar(CalendarMode::Week)),
        Shown {
            mode: CalendarMode::Week,
            anchor: "2026-10-26".to_owned(),
            columns: 7,
        }
    );
    assert_eq!(Shown::of(&calendar(CalendarMode::WorkWeek)).columns, 5);
}

#[test]
fn a_day_and_three_days_start_on_the_day_in_focus() {
    let one = Shown::of(&calendar(CalendarMode::Day));
    assert_eq!((one.anchor.as_str(), one.columns), ("2026-10-28", 1));
    let three = Shown::of(&calendar(CalendarMode::ThreeDay));
    assert_eq!((three.anchor.as_str(), three.columns), ("2026-10-28", 3));
}

#[test]
fn the_month_and_the_agenda_name_the_day_in_focus_and_no_columns() {
    for mode in [CalendarMode::Month, CalendarMode::Agenda] {
        let shown = Shown::of(&calendar(mode));
        assert_eq!((shown.anchor.as_str(), shown.columns), ("2026-10-28", 0));
    }
}

#[test]
fn the_query_names_the_layout_the_grid_draws() {
    let mut side = CalendarSide::default();
    let query = side.query(&calendar(CalendarMode::WorkWeek));
    assert!(matches!(query.layout, CalendarLayout::WorkWeek));
    assert_eq!(query.anchor, "2026-10-26");
    assert_eq!(query.month, None);
}

#[test]
fn a_paged_month_holds_until_the_grid_shows_other_days() {
    let mut side = CalendarSide::default();
    let week = calendar(CalendarMode::Week);
    side.query(&week);
    side.month = Some("2026-12-01".to_owned());

    assert_eq!(
        side.query(&week).month.as_deref(),
        Some("2026-12-01"),
        "a pull with the grid where it was keeps the page"
    );

    let mut moved = calendar(CalendarMode::Week);
    moved.week_anchor = day(19);
    assert_eq!(
        side.query(&moved).month,
        None,
        "the grid moved, so the mini month follows it"
    );
    assert_eq!(side.month, None);

    side.month = Some("2026-12-01".to_owned());
    assert_eq!(
        side.query(&calendar(CalendarMode::Day)).month,
        None,
        "another layout shows other days"
    );
}

fn mini_day(is_today: bool, shown: ShownSpan) -> MiniDay {
    MiniDay {
        date: "2026-10-28".to_owned(),
        day: 28,
        in_month: true,
        is_today,
        shown,
    }
}

#[test]
fn a_day_says_its_date_then_whether_it_is_today_and_shown() {
    let plain = spoken(&mini_day(false, ShownSpan::Outside));
    assert!(!plain.contains(l10n::calendar_today()));
    assert!(!plain.contains(l10n::a11y_calendar_pane_day_shown()));
    assert!(plain.contains("28") && plain.contains("2026"), "{plain}");

    let today_shown = spoken(&mini_day(true, ShownSpan::Middle));
    assert!(today_shown.starts_with(&plain), "{today_shown}");
    assert!(today_shown.ends_with(&format!(
        "{}, {}",
        l10n::calendar_today(),
        l10n::a11y_calendar_pane_day_shown()
    )));
}

#[test]
fn every_part_of_the_band_is_drawn_differently() {
    let classes = [
        ShownSpan::Outside,
        ShownSpan::Only,
        ShownSpan::Start,
        ShownSpan::Middle,
        ShownSpan::End,
    ]
    .map(band_class);
    let distinct = classes.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), classes.len());
}

#[test]
fn arrow_keys_move_by_a_day_and_by_a_week_and_stop_at_the_edge() {
    assert_eq!(arrow_target(8, Key::Right, false), Some(9));
    assert_eq!(arrow_target(8, Key::Left, false), Some(7));
    assert_eq!(arrow_target(8, Key::Up, false), Some(1));
    assert_eq!(arrow_target(8, Key::Down, false), Some(15));
    assert_eq!(arrow_target(0, Key::Left, false), Some(0));
    assert_eq!(arrow_target(3, Key::Up, false), Some(3));
    assert_eq!(arrow_target(41, Key::Right, false), Some(41));
    assert_eq!(arrow_target(38, Key::Down, false), Some(38));
    assert_eq!(arrow_target(8, Key::Return, false), None);
}

#[test]
fn right_to_left_the_arrows_follow_the_page() {
    assert_eq!(arrow_target(8, Key::Left, true), Some(9));
    assert_eq!(arrow_target(8, Key::Right, true), Some(7));
}

fn group(visible: bool) -> CalendarGroup {
    let swatch = || Swatch {
        background: "#111111".to_owned(),
        text: "#ffffff".to_owned(),
        border: "#222222".to_owned(),
    };
    CalendarGroup {
        account: "alice".to_owned(),
        address: "alice@test.local".to_owned(),
        expanded: true,
        calendars: vec![CalendarRow {
            account: "alice".to_owned(),
            account_address: "alice@test.local".to_owned(),
            id: "work".to_owned(),
            name: "Work".to_owned(),
            color: CalendarColor {
                hex: "#4f5ba6".to_owned(),
                light: swatch(),
                dark: swatch(),
            },
            visible,
            can_write: true,
            can_invite: false,
            meeting_choices: Vec::new(),
            default_meeting: None,
            is_default: false,
        }],
    }
}

#[test]
fn the_list_is_drawn_again_only_when_what_it_shows_changed() {
    let none = HashMap::new();
    assert_eq!(keys(&[group(true)], &none), keys(&[group(true)], &none));
    assert_ne!(keys(&[group(true)], &none), keys(&[group(false)], &none));
    let mut shut = group(true);
    shut.expanded = false;
    assert_ne!(keys(&[group(true)], &none), keys(&[shut], &none));
    let account = group(true).account;
    let marked = HashMap::from([(
        account,
        AccountMark {
            severity: NoticeSeverity::Blocked,
            label: "Sign in again".to_owned(),
            detail: "Sign in again".to_owned(),
        },
    )]);
    assert_ne!(
        keys(&[group(true)], &none),
        keys(&[group(true)], &marked),
        "a notice about the account draws its heading again"
    );
}
