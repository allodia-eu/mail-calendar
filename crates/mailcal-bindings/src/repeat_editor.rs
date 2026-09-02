//! What a repeat editor's controls mean, decided once for every client.

use crate::{RecurrenceChange, RepeatDraft};

/// What a save should send for the repeat rule, or `None` to leave the series exactly as it is.
///
/// `draft` is what the editor holds: `None` when the user chose "does not repeat", which is a
/// [`RecurrenceChange::Clear`] for an event that repeats and nothing at all for one that does
/// not. `was_repeating` is whether the event had a rule before the editor opened.
///
/// Call it with the draft the Save button is about to dispatch. A repeat changed and changed back
/// is not a change, and this is what decides that, against the rule the draft carries, not
/// against the form's own history.
///
/// It is also what puts back the parts of a rule no control models: a monthly series pinned to
/// the month's **last day** keeps that when an edit only moves what ends it. Rebuilding from the
/// four controls alone would write "the 31st" instead, and skip every short month.
#[uniffi::export]
#[must_use]
pub fn repeat_change_of(
    draft: Option<RepeatDraft>,
    was_repeating: bool,
) -> Option<RecurrenceChange> {
    mailcal_account::recurrence_change_of(draft.map(Into::into).as_ref(), was_repeating)
        .map(Into::into)
}
