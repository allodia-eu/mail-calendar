//! What names an event to a write: the reference a drawn target carries, and the scope a
//! delete has to make explicit.
//!
//! Apart from the view model because it is the one thing every surface produces and every
//! write consumes: and because `model.rs` is at its 500-line cap.

/// A stable event reference carried by drawn hit targets and agenda rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EventIdentity {
    pub(crate) account: String,
    pub(crate) key: String,
    /// The occurrence this target drew, as the core minted it; empty when there is none to
    /// name: a one-off event, or an agenda row, which lists the series rather than any one of
    /// its occurrences. It travels with the reference because a detail cannot be asked which
    /// day was clicked until it has been read *for* that day.
    pub(crate) occurrence: String,
}

impl EventIdentity {
    /// Whether a write from here has to ask *This event · All events* first.
    pub(crate) fn asks_about_the_series(&self) -> bool {
        !self.occurrence.is_empty()
    }
}

/// Identity plus the destructive scope the confirmation must make explicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeleteRequest {
    pub(super) identity: EventIdentity,
    pub(super) is_recurring: bool,
}
