//! The learning sheet's controls that are more than a row: the range choice with its day, and the
//! status of a read or a run. Split from `writing_style_learn`, which walks the pages.

use adw::prelude::*;
use mailcal_bindings::LearningProgress;

use super::{signatures::named_row, writing_style::paragraph};
use crate::{
    l10n,
    ui::writing_style_flow::{Range, progress_text},
};

/// The two range choices, and the day the second one ends on.
pub(super) struct RangeChoice {
    pub(super) group: adw::PreferencesGroup,
    pub(super) everything: gtk::CheckButton,
    pub(super) up_to: gtk::CheckButton,
    pub(super) day_row: adw::ActionRow,
    pub(super) day: gtk::MenuButton,
    pub(super) calendar: gtk::Calendar,
}

impl RangeChoice {
    /// Untitled: the page it stands on carries the question.
    pub(super) fn new() -> Self {
        let group = adw::PreferencesGroup::new();
        let everything = gtk::CheckButton::new();
        everything.set_active(true);
        let up_to = gtk::CheckButton::new();
        up_to.set_group(Some(&everything));
        group.add(&choice_row(&everything, l10n::learn_range_all(), None));
        group.add(&choice_row(
            &up_to,
            l10n::learn_range_until(),
            Some(l10n::learn_range_until_hint()),
        ));
        let calendar = gtk::Calendar::new();
        let popover = gtk::Popover::new();
        popover.set_child(Some(&calendar));
        let day = gtk::MenuButton::new();
        day.set_popover(Some(&popover));
        day.set_valign(gtk::Align::Center);
        let day_row = named_row(l10n::learn_range_until_label());
        day_row.add_suffix(&day);
        day_row.set_activatable_widget(Some(&day));
        day_row.set_visible(false);
        group.add(&day_row);
        Self {
            group,
            everything,
            up_to,
            day_row,
            day,
            calendar,
        }
    }

    /// Up to the end of the day on the calendar, where the reader is.
    pub(super) fn up_to_day(&self, zone: &str) -> Option<Range> {
        let date = self.calendar.date();
        Range::up_to(date.year(), date.month(), date.day_of_month(), zone)
    }

    /// The range the choice means now.
    pub(super) fn range(&self, zone: &str) -> Option<Range> {
        if self.everything.is_active() {
            Some(Range::default())
        } else {
            self.up_to_day(zone)
        }
    }
}

/// One choice from a set: a check button in a radio group, named by its row.
pub(super) fn choice_row(
    check: &gtk::CheckButton,
    title: &str,
    hint: Option<&str>,
) -> adw::ActionRow {
    let row = named_row(title);
    if let Some(hint) = hint {
        row.set_subtitle(hint);
    }
    check.set_valign(gtk::Align::Center);
    row.add_prefix(check);
    row.set_activatable_widget(Some(check));
    row
}

/// A status line with a spinner, or with a bar once there is a count to fill it from.
pub(super) struct Running {
    pub(super) root: gtk::Box,
    status: gtk::Label,
    spinner: gtk::Spinner,
    bar: gtk::ProgressBar,
}

impl Running {
    pub(super) fn new(progress: Option<&LearningProgress>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let spinner = gtk::Spinner::new();
        let status = paragraph("");
        status.set_hexpand(true);
        line.append(&spinner);
        line.append(&status);
        root.append(&line);
        let bar = gtk::ProgressBar::new();
        root.append(&bar);
        let running = Self {
            root,
            status,
            spinner,
            bar,
        };
        running.show(progress);
        running
    }

    pub(super) fn show(&self, progress: Option<&LearningProgress>) {
        let (text, fraction) = progress_text(progress);
        self.status.set_text(&text);
        self.spinner.set_visible(fraction.is_none());
        self.spinner.set_spinning(fraction.is_none());
        self.bar.set_visible(fraction.is_some());
        self.bar.set_fraction(fraction.unwrap_or_default());
    }
}

pub(super) fn heading(text: &str) -> gtk::Label {
    let label = paragraph(text);
    label.add_css_class("heading");
    label
}
