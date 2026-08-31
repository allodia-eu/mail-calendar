//! The month grid: which days it covers, which month they belong to, and what lands on each.

use engine_api::TimeZoneId;

use super::{super::grid::Occurrence, MONTH_CELLS, MonthGrid, build};

fn zone() -> TimeZoneId {
    TimeZoneId::iana("Europe/Amsterdam").expect("a real zone")
}

fn date(raw: &str) -> engine_api::CalendarDate {
    raw.parse().expect("a valid date")
}

/// A timed occurrence, from `start` to `end` (RFC 3339 UTC).
fn timed(title: &str, start: &str, end: &str) -> Occurrence {
    Occurrence {
        account: "acct".to_owned(),
        event: format!("ev-{title}"),
        calendar: "cal".to_owned(),
        title: title.to_owned(),
        start: start.parse().expect("a valid instant"),
        end: end.parse().expect("a valid instant"),
        all_day: false,
        can_write: true,
        can_move: true,
        occurrence_start: String::new(),
        // Most cases here are about layout, so the default is a settled commitment; the
        // unanswered-hold case sets this explicitly.
        participation: crate::invitation::ResponseStatus::Accepted,
    }
}

/// An all-day occurrence; zoneless, so UTC midnights, end exclusive.
fn all_day(title: &str, start: &str, end: &str) -> Occurrence {
    Occurrence {
        all_day: true,
        ..timed(title, start, end)
    }
}

fn month(anchor: &str, occurrences: &[Occurrence], week_starts_monday: bool) -> MonthGrid {
    build(date(anchor), week_starts_monday, occurrences, &zone())
}

fn titles(grid: &MonthGrid, iso: &str) -> Vec<String> {
    grid.cells
        .iter()
        .find(|cell| cell.date == iso)
        .map(|cell| cell.chips.iter().map(|chip| chip.title.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn a_month_is_always_six_weeks_whatever_month_it_is() {
    // A grid that changes height as you page makes the whole screen jump, and a "+2 more" chip
    // that fits in February and not in March is worse than one that always fits. February 2027
    // starts on a Monday and has 28 days, so it fits in exactly four weeks; it still gets six.
    for anchor in ["2026-07-15", "2027-02-10", "2026-02-01", "2028-01-31"] {
        assert_eq!(
            month(anchor, &[], true).cells.len(),
            MONTH_CELLS,
            "{anchor}"
        );
    }
}

#[test]
fn the_grid_opens_on_the_week_holding_the_first_of_the_month() {
    // July 2026 opens on Wednesday the 1st, so a Monday-start grid begins on Monday 29 June; two
    // days of the previous month, which is exactly what a month grid should show.
    let grid = month("2026-07-15", &[], true);
    assert_eq!(grid.cells[0].date, "2026-06-29");
    assert_eq!(grid.cells[MONTH_CELLS - 1].date, "2026-08-09");
}

#[test]
fn the_week_start_setting_shifts_the_whole_grid() {
    // Sunday-start: the same July opens a day earlier, on Sunday 28 June. Get this wrong and every
    // column of the month shifts, exactly as in the time grid.
    let grid = month("2026-07-15", &[], false);
    assert_eq!(grid.cells[0].date, "2026-06-28");
}

#[test]
fn the_neighbouring_months_days_are_marked_so_a_client_can_dim_them() {
    // Without this the 1st of August looks like part of July, and the user clicks into the wrong
    // month without ever noticing.
    let grid = month("2026-07-15", &[], true);
    let june = grid.cells.iter().find(|c| c.date == "2026-06-30").unwrap();
    let july = grid.cells.iter().find(|c| c.date == "2026-07-01").unwrap();
    let august = grid.cells.iter().find(|c| c.date == "2026-08-01").unwrap();
    assert!(!june.in_month);
    assert!(july.in_month);
    assert!(!august.in_month);
    // And exactly the 31 days of July are in-month.
    assert_eq!(grid.cells.iter().filter(|c| c.in_month).count(), 31);
}

#[test]
fn a_day_reads_top_to_bottom_the_way_it_happens() {
    // All-day first (it bounds the whole day, so it reads as its heading) then timed events by
    // start. A day that lists the 5pm before the 9am is a day you have to re-read.
    let occurrences = vec![
        timed("Evening", "2026-07-15T17:00:00Z", "2026-07-15T18:00:00Z"),
        timed("Morning", "2026-07-15T07:00:00Z", "2026-07-15T08:00:00Z"),
        all_day("Holiday", "2026-07-15T00:00:00Z", "2026-07-16T00:00:00Z"),
    ];
    assert_eq!(
        titles(&month("2026-07-15", &occurrences, true), "2026-07-15"),
        vec!["Holiday", "Morning", "Evening"]
    );
}

#[test]
fn a_multi_day_event_appears_on_every_day_it_covers() {
    // And on the days it merely runs *through*, it starts at midnight: not at its own start, which
    // was on a different day and would read as a lie on this one.
    let offsite = all_day("Offsite", "2026-07-15T00:00:00Z", "2026-07-18T00:00:00Z");
    let grid = month("2026-07-15", std::slice::from_ref(&offsite), true);
    for iso in ["2026-07-15", "2026-07-16", "2026-07-17"] {
        assert_eq!(titles(&grid, iso), vec!["Offsite"], "{iso}");
    }
    // The end is exclusive: a three-day event does not bleed into a fourth.
    assert!(titles(&grid, "2026-07-18").is_empty());

    // A timed event crossing LOCAL midnight lands on both days, and starts at midnight on the
    // second, because it did not begin again that morning.
    //
    // The zone is what decides this, not the UTC clock: 20:00Z–02:00Z is 22:00–04:00 in Amsterdam
    // (UTC+2 in summer), so it genuinely crosses. A naive fixture like 22:00Z–06:00Z would NOT;
    // that is 00:00–08:00 local, one single day, which is exactly the sort of thing that makes a
    // green test meaningless.
    let overnight = timed("Redeye", "2026-07-21T20:00:00Z", "2026-07-22T02:00:00Z");
    let grid = month("2026-07-15", std::slice::from_ref(&overnight), true);
    let first = grid.cells.iter().find(|c| c.date == "2026-07-21").unwrap();
    let second = grid.cells.iter().find(|c| c.date == "2026-07-22").unwrap();
    assert_eq!(first.chips.len(), 1);
    assert_eq!(first.chips[0].start_minutes, 22 * 60, "22:00 local");
    assert_eq!(second.chips.len(), 1);
    assert_eq!(second.chips[0].start_minutes, 0, "the day it runs through");
}

#[test]
fn an_all_day_event_is_zoneless_and_does_not_bleed_into_the_next_day() {
    // The bug this exists to prevent: localising an all-day event drags its UTC midnight to 02:00
    // in Amsterdam, the exclusive end stops looking like a midnight, and EVERY one-day event
    // renders two days wide.
    let holiday = all_day("Holiday", "2026-07-15T00:00:00Z", "2026-07-16T00:00:00Z");
    let grid = month("2026-07-15", std::slice::from_ref(&holiday), true);
    assert_eq!(titles(&grid, "2026-07-15"), vec!["Holiday"]);
    assert!(
        titles(&grid, "2026-07-16").is_empty(),
        "a one-day holiday must not cover two days"
    );
}

#[test]
fn a_cell_carries_every_event_rather_than_a_truncated_list() {
    // How many chips fit is a question of how tall a cell is on this screen: a client concern. The
    // core handing back a pre-truncated list would force every client to the same row height.
    let occurrences: Vec<Occurrence> = (0..9)
        .map(|hour| {
            timed(
                &format!("Meeting {hour}"),
                &format!("2026-07-15T{:02}:00:00Z", hour + 6),
                &format!("2026-07-15T{:02}:30:00Z", hour + 6),
            )
        })
        .collect();
    let grid = month("2026-07-15", &occurrences, true);
    assert_eq!(titles(&grid, "2026-07-15").len(), 9);
}
