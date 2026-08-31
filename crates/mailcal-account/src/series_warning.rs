//! What a client must say before it lets the user edit a whole series.
//!
//! Every transport folds a per-occurrence change into the same overrides map, so the user sees
//! one idea: *this Tuesday is different*. What a later **series** edit does to that difference
//! is four different server policies, and two of them throw the user's work away with nothing
//! on screen to say so. Only a warning at that moment can save it.
//!
//! # Two facts, ANDed, and the second is what keeps it worth reading
//!
//! The account's capability says what the server does. Whether *this* series has any overrides
//! says whether the user has anything to lose. A warning on a clean series is noise, and noise
//! is what teaches people to click past the one that mattered: so the core ANDs them and a
//! client renders whatever it is handed, with no rule of its own to get wrong.
//!
//! # Why a closed enum and not the three booleans
//!
//! A client would have to turn the booleans into a sentence, four times, and the four would
//! disagree. The decision is the core's; the words are the client's (`AGENTS.md` →
//! "Localisation is client-side"). One variant, one catalog key.
//!
//! **No client learns a provider's name.** "Outlook does this" is not a thing to tell a user
//! about their own calendar, and it stops being true the moment a fifth transport arrives.

use engine_api::OverrideSurvival;
use engine_core::calendar::Event;

use crate::{EventEdit, calendar_drag::own_bounds};

/// What editing this whole series costs the occurrences the user changed individually.
///
/// `None` from [`series_edit_warning`] means there is nothing to say; either the server keeps
/// every override, or the user has not changed any occurrence of this series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesEditWarning {
    /// Occurrences the user moved go back to the series' own times.
    OccurrencesReset,
    /// Renaming the series also renames the occurrences the user renamed.
    RenamesSpread,
    /// Both: the moves are undone **and** the names are overwritten.
    OccurrencesResetAndRenamesSpread,
}

/// What a pending series edit actually changes.
///
/// The third fact, and the one that keeps the warning true rather than merely rare. Each
/// `OverrideSurvival` flag describes the consequence of a **particular** kind of edit, so a
/// warning owed for one of them is not owed for the others: on Graph a retitle costs nothing,
/// and saying otherwise announces a loss that will not happen, which spends the user's
/// attention on the transport the warning was built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SeriesEditTouches {
    /// The series' own start or end moves.
    pub timing: bool,
    /// The repeat rule is replaced or cleared.
    pub rule: bool,
    /// A property an override may have set for itself; its title, location or notes.
    pub fields: bool,
}

/// The warning to show before a series-level edit, or `None` when there is none to give.
///
/// `survival` is the account's capability (`None` when the transport cannot write calendars at
/// all); `has_overrides` is whether *this* series holds any per-occurrence change; `touches` is
/// what the edit in the user's hands actually changes.
///
/// The mapping is total over the facts rather than a lookup of the transports that exist
/// today, so a fifth one gets a warning that is true of it rather than the nearest one that was
/// measured. What it cannot do is invent a *reason*: an adapter that destroys overrides some
/// other way needs its own variant, and the pairing below is where that gets noticed.
#[must_use]
pub fn series_edit_warning(
    survival: Option<OverrideSurvival>,
    has_overrides: bool,
    touches: SeriesEditTouches,
) -> Option<SeriesEditWarning> {
    if !has_overrides {
        return None;
    }
    let survival = survival?;
    // Two independent consequences: the user's moves undone, and the user's names overwritten.
    // They are separate sentences because they are separate losses; one is a time going back,
    // the other is a title being replaced, and a client that showed the wrong one would be
    // telling the user something untrue about their own calendar.
    //
    // Each is owed only when the edit does the thing that causes it. Pairing the flag with the
    // change is the whole point: `survives_time_change` says what moving the master costs, and
    // an edit that moves nothing cannot cost it.
    let reset = (touches.timing && !survival.survives_time_change)
        || (touches.rule && !survival.survives_rule_change);
    Some(
        match (reset, touches.fields && survival.clobbers_own_fields) {
            (true, true) => SeriesEditWarning::OccurrencesResetAndRenamesSpread,
            (true, false) => SeriesEditWarning::OccurrencesReset,
            (false, true) => SeriesEditWarning::RenamesSpread,
            (false, false) => return None,
        },
    )
}

/// What `edit` changes about `stored`, as the third fact [`series_edit_warning`] needs.
///
/// Read here rather than in each client for two reasons. A client would be deciding it from the
/// form it seeded, so re-typing a title unchanged would count as a change; this compares against
/// what is **stored**, which is what the server will compare against too. And four clients
/// deriving three booleans is four chances to get one wrong, on a dialog no harness can raise
/// (`docs/calendar.md` → "Known gaps"): so the answer, like the sentence's choice, is the
/// core's.
///
/// A field the edit leaves alone is not a change, and neither is one set to what it already
/// says: the three-state `Option` is "leave it" / "clear it" / "set it", and only the last two
/// can differ.
#[must_use]
pub fn series_edit_touches(stored: &Event, edit: &EventEdit) -> SeriesEditTouches {
    SeriesEditTouches {
        timing: moves(stored, edit),
        // `Set` and `Clear` both replace the rule. A `Set` carrying the rule the series already
        // has would not, but no client writes a rule yet, so the narrower comparison would be
        // untested code guarding a case that cannot arise.
        rule: edit.recurrence.is_some(),
        fields: changes(
            &stored.title,
            edit.title.as_deref().filter(|t| !t.is_empty()),
        ) || changes(
            &stored.description.clone().unwrap_or_default(),
            edit.notes.as_deref(),
        ) || changes(&location_of(stored), edit.location.as_deref()),
    }
}

/// Whether `edit` moves either of the series' own edges.
///
/// A start or end the edit does not name is one it leaves alone. Times the stored event cannot
/// state at all count as a move: the warning is about work that cannot be recovered, so an
/// unanswerable comparison is answered on the side that asks.
fn moves(stored: &Event, edit: &EventEdit) -> bool {
    let Ok((start, end)) = own_bounds(stored) else {
        return true;
    };
    edit.start.is_some_and(|proposed| proposed != start)
        || edit.end.is_some_and(|proposed| proposed != end)
}

/// Whether a three-state edit of one text property changes it.
fn changes(current: &str, proposed: Option<&str>) -> bool {
    proposed.is_some_and(|proposed| proposed != current)
}

/// The event's location as the detail reports it: the first one that has a name.
fn location_of(stored: &Event) -> String {
    stored
        .locations
        .iter()
        .find_map(|location| location.name.clone())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "series_warning_tests.rs"]
mod tests;
