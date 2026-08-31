//! The write behind a drag on the grid.
//!
//! Thin on purpose. The arithmetic that turns a gesture into an event's own wall clock is pure
//! and lives in [`mailcal_account::apply_event_drag`]; the write itself is the *same*
//! provider-neutral patch [`App::update_event`](crate::App) already drives for the editor. What
//! is left here is the one thing neither of those can do: read the stored event, and refuse the
//! drag if it is not the user's to make.
//!
//! Split from `calendar_ops.rs`, which is at the 500-line limit.

use engine_api::Provider;
use mailcal_account::{EventDrag, apply_event_drag};

use crate::{App, reference::EventRef};

impl<P: Provider> App<P> {
    /// Applies a drag to a stored event: shift its start, its end, or both, then patch it.
    ///
    /// **The organiser rule is enforced here, not only in the client.** A client hides the
    /// gesture on a block whose `can_move` is `false`, which is the right thing for the user;
    /// but the intent crosses an FFI, and a write that trusts its caller is not a check. So the
    /// same predicate runs again against the account's own address set: the event must be one
    /// nobody was invited to, or one this account organises. Anything else belongs to somebody
    /// else, and re-timing it silently is what *propose a new time* exists to do politely
    /// (`docs/calendar.md` §13).
    ///
    /// Returns its failure rather than swallowing it, exactly as `update_event` does: the write
    /// is awaited inline with no outbox behind it, so a failed drag is a failed drag and
    /// the caller must not report it as saved.
    ///
    /// # Errors
    ///
    /// Returns the reason the move did not happen: the event is not in the store, it is not the
    /// user's to move, the drag is out of range, or the underlying patch failed.
    pub(super) async fn move_event(
        &self,
        event: &EventRef,
        drag: &EventDrag,
    ) -> Result<(), String> {
        let Some(stored) = self.stored_event(event).await else {
            return Err(format!("no event {:?} in the store", event.key.as_str()));
        };

        let addresses = self.account_address_set(&event.account).await;
        if !crate::invitations::owns_or_organizes(&stored, &addresses) {
            // Not a user-facing error: a client that gates on `can_move` can never reach this,
            // and one that ignores the flag has asked for something the product does not offer.
            // Named by shape, never by content: no title, no organiser, no address.
            return Err("the event is not this account's to move".to_owned());
        }

        let edit = apply_event_drag(&stored, drag).map_err(|err| err.to_string())?;
        self.update_event(event, &edit).await
    }
}
