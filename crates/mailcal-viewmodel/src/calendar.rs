//! The calendar view-models: the agenda list, and the grid a day/week/month view renders.
//!
//! Two shapes over the same data, because they answer different questions. The
//! [`agenda`] is "what is next": a flat, chronological list. The [`grid`] is "what does
//! my week look like": a geometry: which day column, which minutes within it, and, when
//! meetings collide, which lane of which split.
//!
//! The grid's layout is solved **here**, in Rust, and emitted **unit-free**; day
//! indices, minutes, and column fractions, never pixels. Each client multiplies by its
//! own hour height. That split is not tidiness: the moment a drag moves one event into
//! another's slot, the *other* event's column count changes, so whatever re-packs the
//! neighbours has to be whatever holds the pending edit. Solving layout client-side would
//! mean reimplementing the packer in every client, and three greedy packers disagree on
//! the interesting cases almost immediately, invisibly, until someone compares two
//! screens.

pub mod agenda;
/// The shared colour palette, re-exported here because a calendar colour is the surface it
/// was written for and `CalendarColor`'s name hangs off this path across the FFI. It lives
/// at [`crate::color`] now that avatars draw from the same palette.
pub use crate::color;
pub mod days;
pub mod grid;
pub mod month;
pub mod packing;

// The agenda is what this module started as; keep its types reachable at the original
// path so every existing caller (and the FFI) still resolves.
pub use agenda::{AccountEvent, CalendarSnapshot, EventRow, build};

/// One calendar a user can see, colour, and toggle: the row a calendar manager lists and
/// the grid colours its blocks from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRow {
    /// The owning account's id.
    pub account: String,
    /// The calendar's provider key, unique within its account.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The resolved colour, for both themes.
    pub color: color::CalendarColor,
    /// Whether its events are currently drawn.
    pub visible: bool,
    /// Whether this account's calendar provider supports writes. The host uses this to
    /// hide edit affordances on read-only calendars.
    pub can_write: bool,
    /// Whether a new event lands here unless the user picks another calendar.
    ///
    /// The **effective** default, already resolved against what exists: the user's stored choice
    /// while it is still present and still writable, otherwise the first writable calendar.
    /// Exactly one row carries it whenever any calendar can be written to, and none when none
    /// can: so a client reads it rather than keeping a fallback rule of its own.
    pub is_default: bool,
}
