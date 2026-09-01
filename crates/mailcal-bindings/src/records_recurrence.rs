//! The repeat rule across the FFI.
//!
//! A structured rule rather than a frequency word, so a client can seed a repeat editor from
//! it and write a summary that says what the rule actually does; "every 2 weeks on Mon and
//! Thu, until 3 December" rather than "Weekly".
//!
//! The summary itself is **client-side**, like every other piece of localised text: the core
//! emits the structure and each platform assembles the sentence from its own catalog.
//!
//! [`EventRecurrence::Complex`] is the case that matters. It means the event repeats on a rule
//! richer than this shape can hold, so a client says that it repeats and offers **no** edit;
//! seeding an editor from a partial picture would rewrite the user's rule without the parts it
//! could not see.

/// How often an event repeats.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RecurrenceFrequency {
    /// Every day.
    Daily,
    /// Every week.
    Weekly,
    /// Every month.
    Monthly,
    /// Every year.
    Yearly,
}

/// A weekday a rule names.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RecurrenceWeekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// One weekday of a rule, optionally pinned to its nth occurrence in the period.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RecurrenceDay {
    /// The weekday.
    pub day: RecurrenceWeekday,
    /// Which one within the period; `1` is the first, `-1` the last, `None` every one.
    /// "The fourth Monday of the month" is `Monday` with `4`.
    pub nth: Option<i32>,
}

/// When a repeat stops.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RecurrenceEnd {
    /// It does not.
    Never,
    /// On a wall clock in the event's own zone, inclusive (`YYYY-MM-DDTHH:MM:SS`): the same
    /// form as [`EventDetail::start`](crate::EventDetail::start).
    OnDate {
        /// The last wall clock an instance may start at.
        date: String,
    },
    /// After a fixed number of instances, **counting the first**.
    AfterCount {
        /// How many instances in total.
        count: u32,
    },
}

/// A repeat rule a client can both show and offer for editing.
///
/// The `days`, `month_days` and `months` lists are empty when the rule takes that part from
/// the event's own start: a weekly event that simply repeats on its start's weekday names no
/// weekday. A client generating presets ("Monthly on the fourth Monday") reads the start, not
/// this record, which is why an empty list is not a gap to fill in.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SimpleRecurrence {
    /// The base frequency.
    pub frequency: RecurrenceFrequency,
    /// How many periods between instances; `1` is every period, and it is never `0`.
    pub interval: u32,
    /// The weekdays named.
    pub days: Vec<RecurrenceDay>,
    /// The days of the month named; a negative counts from the end of the month.
    pub month_days: Vec<i32>,
    /// The months named, 1–12.
    pub months: Vec<u32>,
    /// When it stops.
    pub end: RecurrenceEnd,
}

/// The repeat rule as an editor's **controls** hold it.
///
/// Four controls (a frequency, how many periods to skip, which weekdays, and what ends it),
/// which is less than [`SimpleRecurrence`] can express. The parts they do not model are carried
/// in [`stored`](Self::stored) and put back on save, so an edit that never touched the repeat
/// cannot rewrite it.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RepeatDraft {
    /// The base frequency.
    pub frequency: RecurrenceFrequency,
    /// How many periods between instances; `1` is every period, and it is never `0`.
    pub interval: u32,
    /// The weekdays a **weekly** rule names, in week order. Never empty: a rule naming none
    /// takes the event's own start weekday. Populated whatever the frequency, so switching to
    /// weekly has a day already ticked; read only when the frequency is weekly.
    pub weekdays: Vec<RecurrenceWeekday>,
    /// What ends it.
    pub end: RecurrenceEnd,
    /// The rule this draft was seeded from, **as the controls hold it**, or `None` when the
    /// event does not repeat yet.
    ///
    /// **Set by the core; pass it back untouched.** It is what tells a rule that changed from one
    /// that did not, and what keeps the parts no control models.
    pub stored: Option<SimpleRecurrence>,
}

impl From<mailcal_account::RepeatDraft> for RepeatDraft {
    fn from(draft: mailcal_account::RepeatDraft) -> Self {
        Self {
            frequency: draft.frequency.into(),
            interval: draft.interval,
            weekdays: draft.weekdays.into_iter().map(Into::into).collect(),
            end: draft.end.into(),
            stored: draft.stored.map(Into::into),
        }
    }
}

impl From<RepeatDraft> for mailcal_account::RepeatDraft {
    fn from(draft: RepeatDraft) -> Self {
        Self {
            frequency: draft.frequency.into(),
            interval: draft.interval,
            weekdays: draft.weekdays.into_iter().map(Into::into).collect(),
            end: draft.end.into(),
            stored: draft.stored.map(Into::into),
        }
    }
}

/// What an event's repeat rule is.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum EventRecurrence {
    /// A rule this app can describe in full. A client may seed its repeat editor from it.
    Simple {
        /// The rule.
        rule: SimpleRecurrence,
    },
    /// The event repeats on a rule richer than [`SimpleRecurrence`] holds; several rules, an
    /// exclusion rule, `bySetPosition`, ISO week numbers, a non-Gregorian calendar, or a
    /// repeat measured in hours or finer.
    ///
    /// Say that it repeats; do **not** offer to change it. The core refuses such a write in
    /// any case, because a client is not the thing that gets to decide this.
    Complex,
}

impl From<mailcal_account::EventRecurrence> for EventRecurrence {
    fn from(recurrence: mailcal_account::EventRecurrence) -> Self {
        match recurrence {
            mailcal_account::EventRecurrence::Simple(rule) => Self::Simple { rule: rule.into() },
            mailcal_account::EventRecurrence::Complex => Self::Complex,
        }
    }
}

impl From<mailcal_account::SimpleRecurrence> for SimpleRecurrence {
    fn from(rule: mailcal_account::SimpleRecurrence) -> Self {
        Self {
            frequency: rule.frequency.into(),
            interval: rule.interval,
            days: rule.days.into_iter().map(Into::into).collect(),
            month_days: rule.month_days,
            months: rule.months,
            end: rule.end.into(),
        }
    }
}

impl From<mailcal_account::RecurrenceFrequency> for RecurrenceFrequency {
    fn from(frequency: mailcal_account::RecurrenceFrequency) -> Self {
        match frequency {
            mailcal_account::RecurrenceFrequency::Daily => Self::Daily,
            mailcal_account::RecurrenceFrequency::Weekly => Self::Weekly,
            mailcal_account::RecurrenceFrequency::Monthly => Self::Monthly,
            mailcal_account::RecurrenceFrequency::Yearly => Self::Yearly,
        }
    }
}

impl From<mailcal_account::RecurrenceDay> for RecurrenceDay {
    fn from(day: mailcal_account::RecurrenceDay) -> Self {
        Self {
            day: day.day.into(),
            nth: day.nth,
        }
    }
}

impl From<mailcal_account::RecurrenceWeekday> for RecurrenceWeekday {
    fn from(day: mailcal_account::RecurrenceWeekday) -> Self {
        match day {
            mailcal_account::RecurrenceWeekday::Monday => Self::Monday,
            mailcal_account::RecurrenceWeekday::Tuesday => Self::Tuesday,
            mailcal_account::RecurrenceWeekday::Wednesday => Self::Wednesday,
            mailcal_account::RecurrenceWeekday::Thursday => Self::Thursday,
            mailcal_account::RecurrenceWeekday::Friday => Self::Friday,
            mailcal_account::RecurrenceWeekday::Saturday => Self::Saturday,
            mailcal_account::RecurrenceWeekday::Sunday => Self::Sunday,
        }
    }
}

impl From<mailcal_account::RecurrenceEnd> for RecurrenceEnd {
    fn from(end: mailcal_account::RecurrenceEnd) -> Self {
        match end {
            mailcal_account::RecurrenceEnd::Never => Self::Never,
            mailcal_account::RecurrenceEnd::OnDate { date } => Self::OnDate { date },
            mailcal_account::RecurrenceEnd::AfterCount { count } => Self::AfterCount { count },
        }
    }
}

/// What an edit does to an event's repeat rule.
///
/// Leaving it out of an edit keeps the series exactly as it is, which is not the same as
/// [`Clear`](Self::Clear); turning a repeating event into a single one.
///
/// Only a **series** edit may carry one, and only over a rule this app could describe in
/// full: an event whose rule reads [`EventRecurrence::Complex`] refuses the write, because
/// the editor was seeded from a partial picture and saving it back would drop the rest.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RecurrenceChange {
    /// Replace the rule, or give a one-off event its first one.
    Set {
        /// The rule.
        rule: SimpleRecurrence,
    },
    /// Stop repeating: every occurrence but the first goes.
    Clear,
}

impl From<mailcal_account::RecurrenceChange> for RecurrenceChange {
    fn from(change: mailcal_account::RecurrenceChange) -> Self {
        match change {
            mailcal_account::RecurrenceChange::Set(rule) => Self::Set { rule: rule.into() },
            mailcal_account::RecurrenceChange::Clear => Self::Clear,
        }
    }
}

impl From<RecurrenceChange> for mailcal_account::RecurrenceChange {
    fn from(change: RecurrenceChange) -> Self {
        match change {
            RecurrenceChange::Set { rule } => Self::Set(rule.into()),
            RecurrenceChange::Clear => Self::Clear,
        }
    }
}

impl From<SimpleRecurrence> for mailcal_account::SimpleRecurrence {
    fn from(rule: SimpleRecurrence) -> Self {
        Self {
            frequency: rule.frequency.into(),
            interval: rule.interval,
            days: rule.days.into_iter().map(Into::into).collect(),
            month_days: rule.month_days,
            months: rule.months,
            end: rule.end.into(),
        }
    }
}

impl From<RecurrenceFrequency> for mailcal_account::RecurrenceFrequency {
    fn from(frequency: RecurrenceFrequency) -> Self {
        match frequency {
            RecurrenceFrequency::Daily => Self::Daily,
            RecurrenceFrequency::Weekly => Self::Weekly,
            RecurrenceFrequency::Monthly => Self::Monthly,
            RecurrenceFrequency::Yearly => Self::Yearly,
        }
    }
}

impl From<RecurrenceDay> for mailcal_account::RecurrenceDay {
    fn from(day: RecurrenceDay) -> Self {
        Self {
            day: day.day.into(),
            nth: day.nth,
        }
    }
}

impl From<RecurrenceWeekday> for mailcal_account::RecurrenceWeekday {
    fn from(day: RecurrenceWeekday) -> Self {
        match day {
            RecurrenceWeekday::Monday => Self::Monday,
            RecurrenceWeekday::Tuesday => Self::Tuesday,
            RecurrenceWeekday::Wednesday => Self::Wednesday,
            RecurrenceWeekday::Thursday => Self::Thursday,
            RecurrenceWeekday::Friday => Self::Friday,
            RecurrenceWeekday::Saturday => Self::Saturday,
            RecurrenceWeekday::Sunday => Self::Sunday,
        }
    }
}

impl From<RecurrenceEnd> for mailcal_account::RecurrenceEnd {
    fn from(end: RecurrenceEnd) -> Self {
        match end {
            RecurrenceEnd::Never => Self::Never,
            RecurrenceEnd::OnDate { date } => Self::OnDate { date },
            RecurrenceEnd::AfterCount { count } => Self::AfterCount { count },
        }
    }
}

/// The edit a client is about to save, as the question "what would this cost?" is asked with.
///
/// The same fields as `Intent::UpdateEvent`, in the same three-state form: absent leaves a
/// property alone, an empty string clears it, a value sets it. Build it from the payload the
/// Save button is about to dispatch rather than from the form's state: the two differ exactly
/// when the user changed something and changed it back, and that is not a change.
///
/// There is no `occurrence`: this asks about a **series** edit. One occurrence's edit writes an
/// override of its own and costs no other occurrence anything.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct ProposedEdit {
    /// The new title, or `None`/empty to leave it.
    #[uniffi(default = None)]
    pub title: Option<String>,
    /// The new start wall clock (`2026-07-01T10:00:00`, or `2026-07-01` if all-day), or
    /// `None`/empty to leave it.
    #[uniffi(default = None)]
    pub start: Option<String>,
    /// The new end, same terms as `start`, or `None`/empty to leave it.
    #[uniffi(default = None)]
    pub end: Option<String>,
    /// The new notes: `None` leaves, empty clears, a value sets.
    #[uniffi(default = None)]
    pub notes: Option<String>,
    /// The new location, same three states as `notes`.
    #[uniffi(default = None)]
    pub location: Option<String>,
    /// What happens to the repeat rule, or `None` to leave the series as it is.
    #[uniffi(default = None)]
    pub recurrence: Option<RecurrenceChange>,
}

impl ProposedEdit {
    /// This edit as the account layer states one, or `None` if a wall clock is unparseable.
    ///
    /// `None` is the same answer an unparseable clock gets from `Intent::UpdateEvent`; the
    /// write refuses it too, so the question and the save agree about what is askable.
    pub(crate) fn into_account_edit(self) -> Option<mailcal_account::EventEdit> {
        let local = |value: Option<String>| match value.filter(|value| !value.is_empty()) {
            Some(value) => value.parse::<engine_api::LocalDateTime>().map(Some).ok(),
            None => Some(None),
        };
        Some(mailcal_account::EventEdit {
            title: self.title,
            start: local(self.start)?,
            end: local(self.end)?,
            notes: self.notes,
            location: self.location,
            recurrence: self.recurrence.map(Into::into),
            occurrence: None,
        })
    }
}

/// What editing a whole series would cost the occurrences the user changed individually.
///
/// Answered by `MailcalApp::series_edit_warning` for the edit in hand, and only when there is
/// something to say: this account's server discards the user's per-occurrence work on a series
/// edit, this series holds some, **and** this edit does the thing that would lose it. All three,
/// which is what keeps the warning worth reading and what keeps it true.
///
/// Show it when the user commits a series-level edit; two of the four transports discard that
/// work silently, so it is the only moment anything can be done about it. Each variant is one
/// catalog key: the core decides which applies, the client writes the sentence, and **no
/// client learns a provider's name**; "Outlook does this" is not a thing to tell somebody
/// about their own calendar, and it stops being true the moment a fifth transport arrives.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SeriesEditWarning {
    /// Occurrences the user moved go back to the series' own times.
    OccurrencesReset,
    /// Renaming the series also renames the occurrences the user renamed.
    RenamesSpread,
    /// Both: the moves are undone **and** the names are overwritten.
    OccurrencesResetAndRenamesSpread,
}

impl From<mailcal_account::SeriesEditWarning> for SeriesEditWarning {
    fn from(warning: mailcal_account::SeriesEditWarning) -> Self {
        match warning {
            mailcal_account::SeriesEditWarning::OccurrencesReset => Self::OccurrencesReset,
            mailcal_account::SeriesEditWarning::RenamesSpread => Self::RenamesSpread,
            mailcal_account::SeriesEditWarning::OccurrencesResetAndRenamesSpread => {
                Self::OccurrencesResetAndRenamesSpread
            }
        }
    }
}
