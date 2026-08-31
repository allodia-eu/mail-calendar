// The write-capability gates: which affordances a read-only account keeps, and which it loses.
//
// The core stamps `canWrite` on every calendar record it emits; the client only reads the flag on
// the exact record it is rendering. The policy is cross-client (Android and Windows apply the same
// one): a per-event delete is HIDDEN when its record cannot write, a disabled swipe action is just
// a mystery, while the global "New event" button is DISABLED rather than hidden, so the header
// keeps its shape. Both decisions are plain values here, unit-tested without rendering a view.

import MailcalBindings

/// Whether the page offers "New event" at all: at least one calendar the user can write to.
///
/// An empty list, nothing synced yet, means no: there is nowhere a new event could go. Total and
/// pure.
func calendarSupportsNewEvent(_ calendars: [CalendarRow]) -> Bool {
    calendars.contains { $0.canWrite }
}

extension EventRow {
    /// Whether this agenda row offers a delete: the flag stamped on this exact record.
    var offersDelete: Bool { canWrite }
}
