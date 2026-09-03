//! The repeat rule as an editor's **controls** hold it.
//!
//! [`SimpleRecurrence`] is the rule this app can state in full. It is still richer than the four
//! controls an editor puts on screen (a frequency, how many periods to skip, which weekdays, and
//! what ends it), so this module projects it once more, onto that.
//!
//! # Why the projection is decided here and not in each client
//!
//! Five clients each deciding which rules their editor may open, and each rebuilding a rule from
//! the same four controls, is five sets of disagreements, and only the one a reader happens to be
//! looking at is visible. It is the argument [`summarize_repeat`] already settles for the sentence
//! a rule gets, made about the form beside it.
//!
//! # What the controls cannot hold, they keep
//!
//! A monthly rule pinned to the month's **last day**, or to a weekday's **position** in it, is a
//! rule no control here offers. Rebuilding such a rule from the controls alone would drop that
//! part and write a different series: "the last day of the month" quietly becoming "the 31st".
//!
//! So a draft carries the rule it was read from, and [`rule_from_draft`] keeps the parts no
//! control models (the month days, the months, a weekday's position) for exactly as long as the
//! **frequency is still the one they were read under**. Change monthly to weekly and they go, which
//! is right: a day of the month means nothing in a week.
//!
//! [`summarize_repeat`]: crate::repeat_summary::summarize_repeat

use engine_core::time::CalendarDate;

use crate::{
    recurrence_shape::{
        RecurrenceChange, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday,
        SimpleRecurrence,
    },
    repeat_summary::{RepeatRhythm, start_weekday, summarize_repeat, week_order},
};

/// What an editor's controls hold, plus the rule they were seeded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatDraft {
    /// The base frequency.
    pub frequency: RecurrenceFrequency,
    /// How many periods between instances; never zero, and `1` for every period.
    pub interval: u32,
    /// The weekdays a **weekly** rule names, in week order. Never empty: a rule naming none
    /// takes the event's own start weekday, and that is what the row shows. Carried for every
    /// frequency so switching to weekly has a day already ticked, and read only when weekly.
    pub weekdays: Vec<RecurrenceWeekday>,
    /// What ends it.
    pub end: RecurrenceEnd,
    /// The rule this draft was seeded from, **as the controls hold it**, or `None` when the
    /// event does not repeat yet.
    ///
    /// Set by the core; a client passes it back untouched. It is what lets an edit tell a rule
    /// that changed from one that did not, and what keeps the parts no control models.
    pub stored: Option<SimpleRecurrence>,
}

/// The controls to show for a stored rule, or `None` when the editor may not open it.
///
/// `None` is the answer for any rule the core could not put into a sentence: the same judgement,
/// on the same rules, that leaves [`crate::repeat_summary::RepeatSummary`] absent. A client that
/// cannot say what a rule is must not offer to change it.
#[must_use]
pub fn repeat_draft_of(rule: &SimpleRecurrence, start: CalendarDate) -> Option<RepeatDraft> {
    let summary = summarize_repeat(rule, start)?;
    let mut draft = RepeatDraft {
        frequency: rule.frequency,
        interval: rule.interval,
        weekdays: match summary.rhythm {
            // Already normalised: in week order, and filled from the start when the rule named
            // none.
            RepeatRhythm::Weekly { days, .. } => days,
            _ => vec![start_weekday(start)],
        },
        end: rule.end.clone(),
        stored: Some(rule.clone()),
    };
    // `stored` is the rule as the controls hold it rather than as it arrived, and the two differ
    // in one case: a weekly rule naming no weekday means the start's, which the row shows ticked.
    // Storing the arriving rule would make an untouched save write that implicit day out as a
    // change the user never made. Everything a control does not model is unaffected, since
    // `rule_from_draft` keeps those from `stored` verbatim.
    draft.stored = Some(rule_from_draft(&draft));
    Some(draft)
}

/// The rule a draft stands for.
///
/// The parts no control models are kept from `stored` while the frequency is unchanged, and
/// dropped when it is not; see the module docs.
#[must_use]
pub fn rule_from_draft(draft: &RepeatDraft) -> SimpleRecurrence {
    let kept = draft
        .stored
        .as_ref()
        .filter(|stored| stored.frequency == draft.frequency);
    let mut weekdays = draft.weekdays.clone();
    weekdays.sort_unstable_by_key(|day| week_order(*day));
    weekdays.dedup();
    SimpleRecurrence {
        frequency: draft.frequency,
        interval: draft.interval,
        days: match draft.frequency {
            RecurrenceFrequency::Weekly => weekdays
                .into_iter()
                .map(|day| RecurrenceDay { day, nth: None })
                .collect(),
            _ => kept.map(|stored| stored.days.clone()).unwrap_or_default(),
        },
        month_days: kept
            .map(|stored| stored.month_days.clone())
            .unwrap_or_default(),
        months: kept.map(|stored| stored.months.clone()).unwrap_or_default(),
        end: draft.end.clone(),
    }
}

/// What a save should send for the repeat rule: nothing at all when it is unchanged.
///
/// `draft` is what the editor holds: `None` when the user chose "does not repeat". The three
/// answers are the ones [`RecurrenceChange`] states, plus the fourth that is not an answer:
/// leaving recurrence out of the edit, which keeps the series exactly as it was.
///
/// A field typed and typed back is not a change, so the comparison is against the **stored** rule
/// the draft carries rather than against the form's own history.
#[must_use]
pub fn recurrence_change_of(
    draft: Option<&RepeatDraft>,
    was_repeating: bool,
) -> Option<RecurrenceChange> {
    let Some(draft) = draft else {
        // Nothing to stop is not the same as stopping nothing.
        return was_repeating.then_some(RecurrenceChange::Clear);
    };
    let rule = rule_from_draft(draft);
    if draft.stored.as_ref() == Some(&rule) {
        return None;
    }
    Some(RecurrenceChange::Set(rule))
}

#[cfg(test)]
#[path = "repeat_draft_tests.rs"]
mod repeat_draft_tests;
