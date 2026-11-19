//! The pane crosses the FFI intact: the weekday order, the band and the groups.

use mailcal_viewmodel::calendar::pane::{self, PaneView};

use super::{CalendarGroup, MiniMonth, ShownSpan};
use crate::RecurrenceWeekday;

#[test]
fn a_sunday_start_mini_month_keeps_its_order_and_its_band() {
    let view = PaneView::Days {
        from: "2026-11-18".parse().unwrap(),
        columns: 3,
    };
    let month: MiniMonth =
        pane::mini_month(&view, "2026-11-18".parse().unwrap(), None, false).into();

    assert_eq!(month.weekdays.first(), Some(&RecurrenceWeekday::Sunday));
    assert_eq!(month.weekdays.last(), Some(&RecurrenceWeekday::Saturday));
    assert_eq!(month.weeks.len(), 6);
    let band: Vec<(String, ShownSpan)> = month
        .weeks
        .into_iter()
        .flat_map(|week| week.days)
        .filter(|day| day.shown != ShownSpan::Outside)
        .map(|day| (day.date, day.shown))
        .collect();
    assert_eq!(
        band,
        vec![
            ("2026-11-18".to_owned(), ShownSpan::Start),
            ("2026-11-19".to_owned(), ShownSpan::Middle),
            ("2026-11-20".to_owned(), ShownSpan::End),
        ]
    );
}

#[test]
fn a_group_carries_its_heading_and_its_expansion() {
    let group: CalendarGroup = pane::CalendarGroup {
        account: "acct".to_owned(),
        address: "me@acct.test".to_owned(),
        expanded: false,
        calendars: Vec::new(),
    }
    .into();
    assert_eq!(group.account, "acct");
    assert_eq!(group.address, "me@acct.test");
    assert!(!group.expanded);
}
