//! The calendar pane as drawn (`docs/calendar-pane.md`): the mini month's days and band, what its
//! presses ask for, and the calendars list. Run from the crate's one GTK test.

use std::{cell::Cell, collections::HashMap, rc::Rc, time::Instant};

use adw::prelude::*;
use mailcal_bindings::{
    CalendarColor, CalendarGroup, CalendarPane, CalendarRow, MiniDay, MiniMonth, MiniWeek,
    NoticeSeverity, RecurrenceWeekday, ShownSpan, Swatch,
};
use time::{Date, Duration, Month};

use super::side_pane::{CalendarSide, PaneInput, SidePane};
use crate::{
    l10n,
    ui::{
        AppInput, PrimaryView, destinations::DestinationBar, icons, notice_rules,
        notice_rules::AccountMark, shell_sidebar,
    },
};

const TODAY: &str = "2026-10-28";
/// The focus date: not today, so its ring is told from today's fill.
const FOCUS: &str = "2026-10-30";

/// October 2026 from Monday 28 September, with the grid on the week of the 26th.
fn october() -> MiniMonth {
    let first = Date::from_calendar_date(2026, Month::September, 28).unwrap();
    let weeks = (0..6_i64)
        .map(|week| MiniWeek {
            number: 40 + u32::try_from(week).unwrap(),
            days: (0..7_i64)
                .map(|offset| {
                    let date = first + Duration::days(week * 7 + offset);
                    let shown = match date.to_string().as_str() {
                        "2026-10-26" => ShownSpan::Start,
                        "2026-11-01" => ShownSpan::End,
                        "2026-10-27" | "2026-10-28" | "2026-10-29" | "2026-10-30"
                        | "2026-10-31" => ShownSpan::Middle,
                        _ => ShownSpan::Outside,
                    };
                    MiniDay {
                        date: date.to_string(),
                        day: u32::from(date.day()),
                        in_month: date.month() == Month::October,
                        is_today: date.to_string() == TODAY,
                        is_focus: date.to_string() == FOCUS,
                        shown,
                    }
                })
                .collect(),
        })
        .collect();
    MiniMonth {
        first: "2026-10-01".to_owned(),
        previous: "2026-09-01".to_owned(),
        next: "2026-11-01".to_owned(),
        weekdays: vec![
            RecurrenceWeekday::Monday,
            RecurrenceWeekday::Tuesday,
            RecurrenceWeekday::Wednesday,
            RecurrenceWeekday::Thursday,
            RecurrenceWeekday::Friday,
            RecurrenceWeekday::Saturday,
            RecurrenceWeekday::Sunday,
        ],
        weeks,
    }
}

fn calendar(id: &str, visible: bool, can_write: bool) -> CalendarRow {
    let swatch = || Swatch {
        background: "#111111".to_owned(),
        text: "#ffffff".to_owned(),
        border: "#222222".to_owned(),
    };
    CalendarRow {
        account: "alice".to_owned(),
        account_address: "alice@test.local".to_owned(),
        id: id.to_owned(),
        name: id.to_owned(),
        color: CalendarColor {
            hex: "#4f5ba6".to_owned(),
            light: swatch(),
            dark: swatch(),
        },
        visible,
        can_write,
        can_invite: false,
        meeting_choices: Vec::new(),
        default_meeting: None,
        is_default: false,
    }
}

fn groups(expanded: bool, work_visible: bool) -> Vec<CalendarGroup> {
    vec![CalendarGroup {
        account: "alice".to_owned(),
        address: "alice@test.local".to_owned(),
        expanded,
        calendars: vec![
            calendar("Work", work_visible, true),
            calendar("Holidays", true, false),
        ],
    }]
}

fn side(expanded: bool, work_visible: bool) -> CalendarSide {
    let mut side = CalendarSide::default();
    side.set_pane(CalendarPane {
        month: october(),
        groups: groups(expanded, work_visible),
    });
    side
}

/// What the pane asked for since the last call: exactly one thing. A marker sent after it makes
/// a press that asked for nothing fail here rather than wait for ever.
#[track_caller]
fn pane_input(sender: &relm4::Sender<AppInput>, receiver: &relm4::Receiver<AppInput>) -> PaneInput {
    sender.emit(AppInput::ToggleAllDay);
    let Some(AppInput::CalendarPane(input)) = receiver.recv_sync() else {
        panic!("the pane asked for nothing");
    };
    assert!(
        matches!(receiver.recv_sync(), Some(AppInput::ToggleAllDay)),
        "the pane asked for one thing"
    );
    input
}

pub(crate) fn the_mini_month_marks_today_and_the_shown_days_and_asks_for_a_seat() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut pane = SidePane::new(&sender);
    pane.render(&side(true, true), &HashMap::new());
    let month = pane.month();

    assert!(month.title().contains("2026"), "{}", month.title());
    let today = month.day(TODAY).expect("today is drawn");
    assert_eq!(today.label().as_deref(), Some("28"));
    assert!(today.has_css_class("mailcal-mini-today"));
    assert!(
        !month
            .day("2026-10-27")
            .unwrap()
            .has_css_class("mailcal-mini-today")
    );
    assert!(
        month
            .day("2026-09-28")
            .unwrap()
            .has_css_class("mailcal-mini-dimmed"),
        "a day of another month is dimmed"
    );
    assert!(!today.has_css_class("mailcal-mini-dimmed"));
    // The focus date is rung, and only it (`docs/calendar.md`, "The focus date").
    let focus = month.day(FOCUS).expect("the focus is drawn");
    assert!(focus.has_css_class("mailcal-mini-focus"));
    assert!(!today.has_css_class("mailcal-mini-focus"));
    assert!(
        month
            .band("2026-10-26")
            .unwrap()
            .has_css_class("mailcal-mini-start")
    );
    assert!(
        month
            .band("2026-10-28")
            .unwrap()
            .has_css_class("mailcal-mini-middle")
    );
    assert!(
        month
            .band("2026-11-01")
            .unwrap()
            .has_css_class("mailcal-mini-end")
    );
    assert!(
        month
            .band("2026-11-02")
            .unwrap()
            .has_css_class("mailcal-mini-outside-band")
    );
    assert!(month.day("2026-11-08").is_some(), "six weeks, always");

    month.day("2026-10-14").unwrap().emit_clicked();
    assert_eq!(
        pane_input(&sender, &receiver),
        PaneInput::Seat("2026-10-14".to_owned())
    );

    let header = pane
        .widget()
        .first_child()
        .and_then(|month| month.first_child())
        .expect("the mini month's header");
    header
        .last_child()
        .and_downcast::<gtk::Button>()
        .expect("the next arrow")
        .emit_clicked();
    assert_eq!(
        pane_input(&sender, &receiver),
        PaneInput::Page("2026-11-01".to_owned())
    );

    // A pull that moves the band relabels the days it has rather than drawing new ones, so the
    // focus on a day stays where it is.
    let mut moved = october();
    for week in &mut moved.weeks {
        for day in &mut week.days {
            day.shown = ShownSpan::Outside;
        }
    }
    moved.weeks[2].days[0].shown = ShownSpan::Only;
    let mut after = CalendarSide::default();
    after.set_pane(CalendarPane {
        month: moved,
        groups: groups(true, true),
    });
    pane.render(&after, &HashMap::new());
    assert_eq!(pane.month().day(TODAY), Some(today));
    assert!(
        pane.month()
            .band("2026-10-12")
            .unwrap()
            .has_css_class("mailcal-mini-only")
    );
    assert!(
        pane.month()
            .band("2026-10-26")
            .unwrap()
            .has_css_class("mailcal-mini-outside-band")
    );
}

pub(crate) fn the_calendars_list_toggles_through_the_core_and_draws_only_what_changed() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut pane = SidePane::new(&sender);
    pane.render(&side(true, true), &HashMap::new());
    let list = pane.calendars().list().clone();
    // On screen, as it is in the app: a button inside a row clicks only once it is realised.
    let window = gtk::Window::new();
    window.set_child(Some(pane.widget()));
    window.present();

    let heading = list.row_at_index(0).expect("the account heading");
    let work = list.row_at_index(1).expect("the Work calendar");
    let holidays = list.row_at_index(2).expect("the Holidays calendar");
    assert!(list.row_at_index(3).is_none());
    assert_eq!(work.accessible_role(), gtk::AccessibleRole::Checkbox);
    assert_eq!(holidays.accessible_role(), gtk::AccessibleRole::Checkbox);
    assert_ne!(heading.accessible_role(), gtk::AccessibleRole::Checkbox);
    assert!(
        images(&holidays).contains(&icons::READ_ONLY.to_owned()),
        "a calendar that cannot be written carries the lock"
    );
    assert!(!images(&work).contains(&icons::READ_ONLY.to_owned()));

    list.emit_by_name::<()>("row-activated", &[&work]);
    assert_eq!(
        pane_input(&sender, &receiver),
        PaneInput::Show {
            account: "alice".to_owned(),
            calendar: "Work".to_owned(),
            visible: false,
        }
    );

    pane.render(&side(true, true), &HashMap::new());
    assert_eq!(
        list.row_at_index(1).as_ref(),
        Some(&work),
        "an unchanged pull keeps its rows"
    );

    pane.render(&side(true, false), &HashMap::new());
    assert_ne!(
        list.row_at_index(1).as_ref(),
        Some(&work),
        "a hidden calendar is drawn again"
    );

    pane.render(&side(false, false), &HashMap::new());
    assert!(list.row_at_index(0).is_some());
    assert!(
        list.row_at_index(1).is_none(),
        "a shut group shows its heading alone"
    );
    // The row answers through its chevron, which GTK plays as a press before it clicks.
    let heading = list.row_at_index(0).unwrap();
    let chevron = descendants::<gtk::Button>(&heading)
        .into_iter()
        .next()
        .expect("the chevron");
    let clicked = Rc::new(Cell::new(false));
    let seen = Rc::clone(&clicked);
    chevron.connect_clicked(move |_| seen.set(true));
    list.emit_by_name::<()>("row-activated", &[&heading]);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !clicked.get() {
        assert!(
            Instant::now() < deadline,
            "the heading never opened its group"
        );
        gtk::glib::MainContext::default().iteration(false);
    }
    assert_eq!(
        pane_input(&sender, &receiver),
        PaneInput::Expand {
            account: "alice".to_owned(),
            expanded: true,
        }
    );
    window.destroy();
}

/// A group's heading wears its account's notice mark as the folder pane's account row does
/// (`docs/calendar-pane.md` rule 7, `docs/notices.md` rule 6), and a heading whose account has
/// no notice wears none.
pub(crate) fn a_calendar_group_heading_carries_its_accounts_notice_mark() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut pane = SidePane::new(&sender);
    let mut bob = groups(false, true).remove(0);
    bob.account = "bob".to_owned();
    bob.address = "bob@test.local".to_owned();
    let mut both = groups(false, true);
    both.push(bob);
    let mut side = side(false, true);
    side.set_pane(CalendarPane {
        month: october(),
        groups: both,
    });
    let marks = HashMap::from([(
        "alice".to_owned(),
        AccountMark {
            severity: NoticeSeverity::Blocked,
            label: l10n::notices_account_count(2),
            detail: "first\nsecond".to_owned(),
        },
    )]);
    pane.render(&side, &marks);
    let list = pane.calendars().list().clone();
    let heading = |index: i32| {
        list.row_at_index(index)
            .and_downcast::<adw::ActionRow>()
            .expect("an account heading")
    };

    let alice = heading(0);
    assert_eq!(
        alice.subtitle().as_deref(),
        Some(l10n::notices_account_count(2).as_str())
    );
    assert_eq!(alice.subtitle_lines(), 1, "never wrapping");
    assert!(
        images(&alice).contains(&notice_rules::glyph(NoticeSeverity::Blocked).to_owned()),
        "the first notice's glyph"
    );
    let bob = heading(1);
    assert_eq!(
        bob.subtitle().unwrap_or_default(),
        "",
        "a healthy account carries no mark"
    );
    assert!(!images(&bob).contains(&notice_rules::glyph(NoticeSeverity::Blocked).to_owned()));

    pane.render(&side, &HashMap::new());
    assert_eq!(
        heading(0).subtitle().unwrap_or_default(),
        "",
        "the mark leaves with the notice"
    );
}

pub(crate) fn the_folder_column_holds_the_calendar_pane_beside_the_calendar_only() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let calendar = SidePane::new(&sender);
    let sidebar = shell_sidebar::sidebar_pane(
        &gtk::ScrolledWindow::new(),
        calendar.widget(),
        &DestinationBar::new(&sender),
    );
    for (view, shown) in [
        (PrimaryView::Calendar, "calendar"),
        (PrimaryView::Mail, "folders"),
        (PrimaryView::Contacts, "folders"),
    ] {
        sidebar.show(view);
        assert_eq!(sidebar.showing().as_deref(), Some(shown), "{view:?}");
    }
}

/// Every icon name drawn inside `widget`.
fn images(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants::<gtk::Image>(widget)
        .iter()
        .filter_map(gtk::Image::icon_name)
        .map(|name| name.to_string())
        .collect()
}

/// Every `T` inside `widget`, in no particular order.
fn descendants<T: IsA<gtk::Widget>>(widget: &impl IsA<gtk::Widget>) -> Vec<T> {
    let mut found = Vec::new();
    let mut stack = vec![widget.clone().upcast::<gtk::Widget>()];
    while let Some(next) = stack.pop() {
        if let Ok(typed) = next.clone().downcast::<T>() {
            found.push(typed);
        }
        let mut child = next.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            stack.push(current);
        }
    }
    found
}
