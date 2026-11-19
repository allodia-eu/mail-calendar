// A per-event delete is HIDDEN when the record it is drawn from cannot write: a disabled swipe
// action is just a mystery. The core stamps `canWrite` on every record it emits; the client reads
// the flag on the exact record it renders. Whether "New event" is offered at all is the core's
// `newEventCalendars()`, read by the toolbar and the grid.

import MailcalBindings

extension EventRow {
    /// Whether this agenda row offers a delete: the flag stamped on this exact record.
    var offersDelete: Bool { canWrite }
}
