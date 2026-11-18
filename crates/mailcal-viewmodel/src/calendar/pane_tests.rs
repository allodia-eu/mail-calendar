//! The calendar pane: which month the mini month shows, which days it marks, the week numbers
//! down its side, and how the calendars group under their accounts.

use super::*;
use crate::calendar::color::resolve;

fn date(raw: &str) -> CalendarDate {
    raw.parse().expect("a valid date")
}

fn days(from: &str, columns: u32) -> PaneView {
    PaneView::Days {
        from: date(from),
        columns,
    }
}

fn shown_of(month: &MiniMonth) -> Vec<(String, ShownSpan)> {
    month
        .weeks
        .iter()
        .flat_map(|week| &week.days)
        .filter(|day| day.shown != ShownSpan::Outside)
        .map(|day| (day.date.clone(), day.shown))
        .collect()
}

#[test]
fn a_grid_showing_today_follows_todays_month() {
    let today = date("2026-12-01");
    // The week runs from November into December; today is in it, so the mini month is December.
    let first = followed_month(&days("2026-11-30", 7), today);
    assert_eq!(first.to_string(), "2026-12-01");
}

#[test]
fn a_grid_away_from_today_follows_its_first_shown_day() {
    let today = date("2026-11-18");
    let first = followed_month(&days("2027-02-27", 7), today);
    assert_eq!(first.to_string(), "2027-02-01");
    // The first shown day decides, even when most of the days are in the next month.
    let first = followed_month(&days("2027-01-31", 7), today);
    assert_eq!(first.to_string(), "2027-01-01");
}

#[test]
fn the_month_layout_shows_its_anchors_month_and_the_agenda_todays() {
    let today = date("2026-11-18");
    let month = PaneView::Month {
        anchor: date("2027-03-14"),
    };
    assert_eq!(followed_month(&month, today).to_string(), "2027-03-01");
    assert_eq!(
        followed_month(&PaneView::Agenda, today).to_string(),
        "2026-11-01"
    );
}

#[test]
fn a_paged_month_overrides_the_followed_one() {
    let today = date("2026-11-18");
    let month = mini_month(
        &days("2026-11-16", 7),
        today,
        Some(date("2027-05-20")),
        true,
    );
    assert_eq!(month.first, "2027-05-01");
    // The grid's week is not in May, so nothing is marked shown.
    assert_eq!(shown_of(&month), []);
}

#[test]
fn the_arrows_carry_the_neighbouring_months_across_a_year() {
    let month = mini_month(&PaneView::Agenda, date("2026-12-10"), None, true);
    assert_eq!(month.first, "2026-12-01");
    assert_eq!(month.previous, "2026-11-01");
    assert_eq!(month.next, "2027-01-01");
    let month = mini_month(&PaneView::Agenda, date("2027-01-10"), None, true);
    assert_eq!(month.previous, "2026-12-01");
}

#[test]
fn always_six_weeks_of_seven_in_the_week_start_order() {
    let month = mini_month(&PaneView::Agenda, date("2026-02-10"), None, true);
    assert_eq!(month.weeks.len(), 6);
    assert!(month.weeks.iter().all(|week| week.days.len() == 7));
    assert_eq!(month.weekdays, vec![0, 1, 2, 3, 4, 5, 6]);
    // February 2026 starts on a Sunday: a Monday-start month opens on 26 January.
    assert_eq!(month.weeks[0].days[0].date, "2026-01-26");
    assert!(!month.weeks[0].days[0].in_month);
    assert!(month.weeks[0].days[6].in_month);

    let sunday = mini_month(&PaneView::Agenda, date("2026-02-10"), None, false);
    assert_eq!(sunday.weekdays, vec![6, 0, 1, 2, 3, 4, 5]);
    assert_eq!(sunday.weeks[0].days[0].date, "2026-02-01");
    assert_eq!(sunday.weeks[0].days[0].day, 1);
}

#[test]
fn week_numbers_are_iso_whatever_the_week_start() {
    // December 2026 into January 2027: 1 January 2027 is still week 53.
    let monday = mini_month(&PaneView::Agenda, date("2026-12-10"), None, true);
    let numbers: Vec<u32> = monday.weeks.iter().map(|week| week.number).collect();
    assert_eq!(numbers, vec![49, 50, 51, 52, 53, 1]);
    let row = monday
        .weeks
        .iter()
        .find(|week| week.days.iter().any(|day| day.date == "2027-01-01"))
        .expect("the row holding 1 January");
    assert_eq!(row.number, 53);

    // A Sunday-start row is numbered by the week its Monday is in.
    let sunday = mini_month(&PaneView::Agenda, date("2026-12-10"), None, false);
    assert_eq!(sunday.weeks[0].days[0].date, "2026-11-29");
    assert_eq!(sunday.weeks[0].number, 49);
}

#[test]
fn today_is_marked_only_on_its_own_day() {
    let month = mini_month(&PaneView::Agenda, date("2026-11-18"), None, true);
    let today: Vec<&str> = month
        .weeks
        .iter()
        .flat_map(|week| &week.days)
        .filter(|day| day.is_today)
        .map(|day| day.date.as_str())
        .collect();
    assert_eq!(today, vec!["2026-11-18"]);
}

#[test]
fn the_shown_days_are_one_band_per_row() {
    // Thursday 19 to Wednesday 25 November: the band wraps the Monday-start row boundary.
    let month = mini_month(&days("2026-11-19", 7), date("2026-11-18"), None, true);
    assert_eq!(
        shown_of(&month),
        vec![
            ("2026-11-19".to_owned(), ShownSpan::Start),
            ("2026-11-20".to_owned(), ShownSpan::Middle),
            ("2026-11-21".to_owned(), ShownSpan::Middle),
            ("2026-11-22".to_owned(), ShownSpan::End),
            ("2026-11-23".to_owned(), ShownSpan::Start),
            ("2026-11-24".to_owned(), ShownSpan::Middle),
            ("2026-11-25".to_owned(), ShownSpan::End),
        ]
    );
}

#[test]
fn a_single_shown_day_is_its_own_capsule() {
    let month = mini_month(&days("2026-11-18", 1), date("2026-11-18"), None, true);
    assert_eq!(
        shown_of(&month),
        vec![("2026-11-18".to_owned(), ShownSpan::Only)]
    );
    // A day alone at a row's end is also whole.
    let month = mini_month(&days("2026-11-22", 2), date("2026-11-18"), None, true);
    assert_eq!(
        shown_of(&month),
        vec![
            ("2026-11-22".to_owned(), ShownSpan::Only),
            ("2026-11-23".to_owned(), ShownSpan::Only),
        ]
    );
}

#[test]
fn the_month_and_agenda_layouts_mark_nothing() {
    let today = date("2026-11-18");
    let month = PaneView::Month { anchor: today };
    assert_eq!(shown_of(&mini_month(&month, today, None, true)), []);
    assert_eq!(
        shown_of(&mini_month(&PaneView::Agenda, today, None, true)),
        []
    );
}

fn row(account: &str, id: &str) -> CalendarRow {
    CalendarRow {
        account: account.to_owned(),
        account_address: format!("me@{account}.test"),
        id: id.to_owned(),
        name: id.to_owned(),
        color: resolve(None, None, 0),
        visible: true,
        can_write: true,
        can_invite: false,
        meeting_choices: Vec::new(),
        default_meeting: None,
        is_default: false,
    }
}

#[test]
fn groups_keep_the_account_order_and_never_sort_by_id() {
    let rows = vec![
        row("zulu", "home"),
        row("alpha", "work"),
        row("zulu", "family"),
    ];
    let grouped = groups(rows, |_| true);
    let accounts: Vec<&str> = grouped.iter().map(|group| group.account.as_str()).collect();
    assert_eq!(accounts, vec!["zulu", "alpha"]);
    assert_eq!(grouped[0].address, "me@zulu.test");
    let zulu: Vec<&str> = grouped[0]
        .calendars
        .iter()
        .map(|calendar| calendar.id.as_str())
        .collect();
    assert_eq!(zulu, vec!["home", "family"]);
}

#[test]
fn a_group_reads_its_expansion_and_an_account_without_calendars_has_none() {
    let grouped = groups(vec![row("a", "x"), row("b", "y")], |account| account != "b");
    assert!(grouped[0].expanded);
    assert!(!grouped[1].expanded);
    assert_eq!(groups(Vec::new(), |_| true), []);
}
