//! The repeat controls inside the event editor: a frequency, how many periods to skip, which
//! weekdays a weekly rule falls on, and what ends it.
//!
//! Four controls, which is less than a rule can say. The parts they do not model (a monthly series
//! pinned to the month's last day, or to a weekday's position in it) ride along in the draft's
//! `stored` rule and are put back by the core, so an edit that never touched the repeat cannot
//! rewrite it. Which rules may be opened at all is the core's answer too:
//! `EventDetail::repeat_draft` is absent for a rule it could not state in full, and then the
//! summary is shown with no controls.
//!
//! The panel rebuilds itself whenever the choice changes, because which rows exist depends on it.
//! The pure half (what a choice is, and the weekday arithmetic) is below the widgets and is what
//! `repeat_editor_tests` drives.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday, RepeatDraft};

use crate::l10n;

/// The draft the form holds while the dialog is open, shared with the widgets that change it.
pub(super) type SharedDraft = Rc<RefCell<Option<RepeatDraft>>>;

/// The most periods, and the most instances, either spinner will go to. Well under the core's own
/// ceiling, which refuses a rule no calendar could draw.
const CEILING: f64 = 999.0;

/// What the frequency picker offers, including the choice not to repeat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RepeatChoice {
    Never,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl RepeatChoice {
    pub(super) const ALL: [Self; 5] = [
        Self::Never,
        Self::Daily,
        Self::Weekly,
        Self::Monthly,
        Self::Yearly,
    ];

    pub(super) const fn of(frequency: Option<&RecurrenceFrequency>) -> Self {
        match frequency {
            None => Self::Never,
            Some(RecurrenceFrequency::Daily) => Self::Daily,
            Some(RecurrenceFrequency::Weekly) => Self::Weekly,
            Some(RecurrenceFrequency::Monthly) => Self::Monthly,
            Some(RecurrenceFrequency::Yearly) => Self::Yearly,
        }
    }

    pub(super) const fn frequency(self) -> Option<RecurrenceFrequency> {
        match self {
            Self::Never => None,
            Self::Daily => Some(RecurrenceFrequency::Daily),
            Self::Weekly => Some(RecurrenceFrequency::Weekly),
            Self::Monthly => Some(RecurrenceFrequency::Monthly),
            Self::Yearly => Some(RecurrenceFrequency::Yearly),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Never => l10n::event_repeat_none(),
            Self::Daily => l10n::event_repeat_daily(),
            Self::Weekly => l10n::event_repeat_weekly(),
            Self::Monthly => l10n::event_repeat_monthly(),
            Self::Yearly => l10n::event_repeat_yearly(),
        }
    }

    /// "Every 3 weeks": the interval spinner's own title. Never the frequency word: the picker
    /// directly above already shows it, and a title repeating it reads as a duplicate rather than
    /// as the period it sets.
    pub(super) fn interval_label(self, interval: u32) -> String {
        let count = i64::from(interval);
        match (self, interval > 1) {
            (Self::Never, _) => self.label().to_owned(),
            (Self::Daily, false) => l10n::event_repeat_every_day().to_owned(),
            (Self::Daily, true) => l10n::event_repeat_sum_daily_n(count),
            (Self::Weekly, false) => l10n::event_repeat_every_week().to_owned(),
            (Self::Weekly, true) => l10n::event_repeat_every_weeks(count),
            (Self::Monthly, false) => l10n::event_repeat_every_month().to_owned(),
            (Self::Monthly, true) => l10n::event_repeat_every_months(count),
            (Self::Yearly, false) => l10n::event_repeat_every_year().to_owned(),
            (Self::Yearly, true) => l10n::event_repeat_every_years(count),
        }
    }
}

/// What the "Ends" picker offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RepeatEndChoice {
    Never,
    OnDate,
    AfterCount,
}

impl RepeatEndChoice {
    pub(super) const ALL: [Self; 3] = [Self::Never, Self::OnDate, Self::AfterCount];

    pub(super) const fn of(end: &RecurrenceEnd) -> Self {
        match end {
            RecurrenceEnd::Never => Self::Never,
            RecurrenceEnd::OnDate { .. } => Self::OnDate,
            RecurrenceEnd::AfterCount { .. } => Self::AfterCount,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Never => l10n::event_repeat_ends_never(),
            Self::OnDate => l10n::event_repeat_ends_on_date(),
            Self::AfterCount => l10n::event_repeat_ends_after_count(),
        }
    }
}

/// The week, Monday first: the order the core counts weekdays in.
pub(super) const WEEK: [RecurrenceWeekday; 7] = [
    RecurrenceWeekday::Monday,
    RecurrenceWeekday::Tuesday,
    RecurrenceWeekday::Wednesday,
    RecurrenceWeekday::Thursday,
    RecurrenceWeekday::Friday,
    RecurrenceWeekday::Saturday,
    RecurrenceWeekday::Sunday,
];

/// Ticks or unticks one weekday, returning the row in week order.
///
/// At least one day stays ticked: a weekly rule that names none is not a rule, and the core would
/// refuse it, so unticking the last one is a no-op.
pub(super) fn toggled(
    current: &[RecurrenceWeekday],
    day: RecurrenceWeekday,
) -> Vec<RecurrenceWeekday> {
    let next: Vec<RecurrenceWeekday> = if current.contains(&day) {
        if current.len() == 1 {
            return current.to_vec();
        }
        current.iter().filter(|d| **d != day).cloned().collect()
    } else {
        current
            .iter()
            .cloned()
            .chain(std::iter::once(day))
            .collect()
    };
    WEEK.into_iter().filter(|d| next.contains(d)).collect()
}

/// The weekday's full name for a screen reader, and its abbreviation for the button. Both come
/// from the process locale rather than the catalog: a weekday name is the one part of a localised
/// string nobody has to translate.
fn weekday_names(day: &RecurrenceWeekday) -> (String, String) {
    let iso = super::repeat::iso_weekday(day);
    (
        super::date::weekday_full(iso),
        super::date::weekday_abbrev(iso),
    )
}

/// The weekday a rule first chosen on this event should fall on, from a `YYYY-MM-DD…` wall clock.
pub(super) fn weekday_of(wall_clock: &str) -> RecurrenceWeekday {
    super::date::date_from_wall(wall_clock).map_or(RecurrenceWeekday::Monday, |date| {
        WEEK[usize::from(date.weekday().number_days_from_monday())].clone()
    })
}

/// The editor's repeat section: the controls when the core handed over a draft, and the sentence
/// it already decided when it did not.
///
/// Returns the draft the save handler reads back out of the widgets.
pub(super) fn append_repeat_section(
    form: &gtk::Box,
    editor: &super::editor::EventEditor,
) -> SharedDraft {
    let draft: SharedDraft = Rc::new(RefCell::new(
        editor
            .editing
            .as_ref()
            .and_then(|detail| detail.repeat_draft.clone()),
    ));
    if editor.can_edit_repeat() {
        form.append(&repeat_group(&draft, &editor.start));
        if editor.asks_about_the_series() {
            form.append(&dim_label(l10n::event_repeat_series_note()));
        }
        return draft;
    }
    let heading = gtk::Label::new(Some(l10n::event_repeat()));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    form.append(&heading);
    let editing = editor.editing.as_ref();
    let summary = gtk::Label::new(Some(&super::repeat::sentence(
        editing.and_then(|detail| detail.repeat_summary.as_ref()),
        editing.is_some_and(|detail| detail.is_recurring),
    )));
    summary.set_wrap(true);
    summary.set_xalign(0.0);
    form.append(&summary);
    form.append(&dim_label(l10n::event_repeat_locked()));
    draft
}

fn dim_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}

/// The repeat controls, over a draft the save handler reads back.
///
/// Every row goes into an `AdwPreferencesGroup`: an `AdwComboRow` or `AdwSpinRow` appended to a
/// plain `GtkBox` still renders, but GTK's focus walk cannot focus it, so the keyboard skips the
/// control entirely.
///
/// The group itself sits in a box of ours, because which rows exist depends on the choice and a
/// group cannot be emptied: its own children are the template widgets rather than the rows it was
/// given, so a change swaps the whole group for a fresh one.
pub(super) fn repeat_group(draft: &SharedDraft, start: &str) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    populate(&container, draft, start);
    container
}

fn populate(container: &gtk::Box, draft: &SharedDraft, start: &str) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let group = adw::PreferencesGroup::new();
    container.append(&group);
    let held = draft.borrow().clone();
    let choice = RepeatChoice::of(held.as_ref().map(|value| &value.frequency));

    let frequency = combo_row(
        l10n::event_repeat(),
        &RepeatChoice::ALL.map(RepeatChoice::label),
    );
    frequency.set_selected(index_of(&RepeatChoice::ALL, &choice));
    group.add(&frequency);
    {
        let draft = Rc::clone(draft);
        let container = container.clone();
        let start = start.to_owned();
        frequency.connect_selected_notify(move |row| {
            let picked = RepeatChoice::ALL[selected_index(row.selected(), RepeatChoice::ALL.len())];
            {
                let mut slot = draft.borrow_mut();
                *slot = picked.frequency().map(|frequency| match slot.clone() {
                    Some(existing) => RepeatDraft {
                        frequency,
                        ..existing
                    },
                    None => RepeatDraft {
                        frequency,
                        interval: 1,
                        weekdays: vec![weekday_of(&start)],
                        end: RecurrenceEnd::Never,
                        stored: None,
                    },
                });
            }
            populate(&container, &draft, &start);
        });
    }

    let Some(held) = held else {
        return;
    };

    let interval = spin_row(
        &choice.interval_label(held.interval),
        f64::from(held.interval),
    );
    group.add(&interval);
    {
        let draft = Rc::clone(draft);
        let container = container.clone();
        let start = start.to_owned();
        interval.connect_value_notify(move |row| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the row is clamped to 1..=999 by its own adjustment"
            )]
            let next = row.value() as u32;
            let mut changed = false;
            {
                let mut slot = draft.borrow_mut();
                if let Some(value) = slot.as_mut()
                    && value.interval != next
                {
                    value.interval = next;
                    changed = true;
                }
            }
            if changed {
                populate(&container, &draft, &start);
            }
        });
    }

    if held.frequency == RecurrenceFrequency::Weekly {
        group.add(&weekday_row(draft, container, start, &held.weekdays));
    }

    let ends = combo_row(
        l10n::event_repeat_ends(),
        &RepeatEndChoice::ALL.map(RepeatEndChoice::label),
    );
    ends.set_selected(index_of(
        &RepeatEndChoice::ALL,
        &RepeatEndChoice::of(&held.end),
    ));
    group.add(&ends);
    {
        let draft = Rc::clone(draft);
        let container = container.clone();
        let start = start.to_owned();
        ends.connect_selected_notify(move |row| {
            let picked =
                RepeatEndChoice::ALL[selected_index(row.selected(), RepeatEndChoice::ALL.len())];
            {
                let mut slot = draft.borrow_mut();
                if let Some(value) = slot.as_mut() {
                    value.end = match picked {
                        RepeatEndChoice::Never => RecurrenceEnd::Never,
                        // A year out: far enough to be a deliberate choice, near enough to reach.
                        RepeatEndChoice::OnDate => RecurrenceEnd::OnDate {
                            date: a_year_after(&start),
                        },
                        RepeatEndChoice::AfterCount => RecurrenceEnd::AfterCount { count: 10 },
                    };
                }
            }
            populate(&container, &draft, &start);
        });
    }

    match &held.end {
        RecurrenceEnd::OnDate { date } => {
            let row = adw::EntryRow::new();
            row.set_use_markup(false);
            row.set_title(l10n::event_repeat_ends_date());
            row.set_text(date.split('T').next().unwrap_or(date));
            group.add(&row);
            let draft = Rc::clone(draft);
            row.connect_changed(move |row| {
                let typed = row.text();
                if let Some(value) = draft.borrow_mut().as_mut() {
                    value.end = RecurrenceEnd::OnDate {
                        date: format!("{typed}T00:00:00"),
                    };
                }
            });
        }
        RecurrenceEnd::AfterCount { count } => {
            let row = spin_row(
                &l10n::event_repeat_ends_times(i64::from(*count)),
                f64::from(*count),
            );
            group.add(&row);
            let draft = Rc::clone(draft);
            let container = container.clone();
            let start = start.to_owned();
            row.connect_value_notify(move |row| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the row is clamped to 1..=999 by its own adjustment"
                )]
                let next = row.value() as u32;
                let mut changed = false;
                {
                    let mut slot = draft.borrow_mut();
                    if let Some(value) = slot.as_mut()
                        && value.end != (RecurrenceEnd::AfterCount { count: next })
                    {
                        value.end = RecurrenceEnd::AfterCount { count: next };
                        changed = true;
                    }
                }
                if changed {
                    populate(&container, &draft, &start);
                }
            });
        }
        RecurrenceEnd::Never => {}
    }
}

fn weekday_row(
    draft: &SharedDraft,
    container: &gtk::Box,
    start: &str,
    ticked: &[RecurrenceWeekday],
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(l10n::event_repeat_weekly());
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    for day in WEEK {
        let (name, short) = weekday_names(&day);
        let button = gtk::ToggleButton::with_label(&short);
        button.set_active(ticked.contains(&day));
        // The button shows an initial; a screen reader gets the whole word.
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        let draft = Rc::clone(draft);
        let container = container.clone();
        let start = start.to_owned();
        button.connect_clicked(move |_| {
            {
                let mut slot = draft.borrow_mut();
                if let Some(value) = slot.as_mut() {
                    value.weekdays = toggled(&value.weekdays, day.clone());
                }
            }
            populate(&container, &draft, &start);
        });
        box_.append(&button);
    }
    row.add_suffix(&box_);
    row
}

fn combo_row(title: &str, labels: &[&str]) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    row.set_model(Some(&gtk::StringList::new(labels)));
    row
}

fn spin_row(title: &str, value: f64) -> adw::SpinRow {
    let row = adw::SpinRow::new(
        Some(&gtk::Adjustment::new(value, 1.0, CEILING, 1.0, 10.0, 0.0)),
        1.0,
        0,
    );
    row.set_use_markup(false);
    row.set_title(title);
    row
}

/// A `GtkDropDown` answers `GTK_INVALID_LIST_POSITION` when nothing is selected, which as an index
/// would panic.
fn selected_index(selected: u32, len: usize) -> usize {
    usize::try_from(selected).unwrap_or(0).min(len - 1)
}

fn index_of<T: PartialEq>(all: &[T], value: &T) -> u32 {
    u32::try_from(all.iter().position(|item| item == value).unwrap_or(0)).unwrap_or(0)
}

/// The same wall-clock day, one year on: the default a rule ending "on a date" opens with.
fn a_year_after(start: &str) -> String {
    super::date::date_from_wall(start).map_or_else(
        || start.to_owned(),
        |date| {
            let next = date.replace_year(date.year() + 1).unwrap_or(date);
            format!("{next}T00:00:00")
        },
    )
}

#[cfg(test)]
#[path = "repeat_editor_tests.rs"]
mod repeat_editor_tests;
