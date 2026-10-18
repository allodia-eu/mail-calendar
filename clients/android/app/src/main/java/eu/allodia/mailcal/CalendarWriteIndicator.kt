// The small in-calendar indicator that tells the user their create/edit/delete took.
//
// This is the Kotlin side of the core's `CalendarWriteStatus` (Surface.CALENDAR_STATUS). The mapping
// is a plain, Compose-free value so the state machine is unit-tested without composing a screen, the
// composable (`CalendarWriteBadge` in CalendarChrome.kt) is a thin render of the result.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.CalendarWriteStatus

/**
 * What the header should show for the most recent calendar write.
 *
 * Both warnings offer a retry, and the retry is a `RefreshCalendar`, never a re-send: a change that
 * was not saved changed nothing on the server, and one that could not be confirmed may have landed,
 * so sending it again could make it twice.
 */
internal enum class CalendarWriteIndicator {
    /** Nothing to show. */
    Hidden,

    /** A write is settling, a small spinner. */
    Spinner,

    /** The write settled and the local view holds the server's copy, a brief check. */
    Saved,

    /** The change was not saved, a warning the user can tap to refresh. */
    NotSaved,

    /** The change may have landed and could not be confirmed, the same warning. */
    Unconfirmed,
    ;

    /** Whether tapping the indicator should trigger a retry (a `RefreshCalendar`). */
    val offersRetry: Boolean
        get() = this == NotSaved || this == Unconfirmed

    companion object {
        /** Maps a core [CalendarWriteStatus] to what the header shows. Total and pure. */
        fun of(status: CalendarWriteStatus): CalendarWriteIndicator =
            when (status) {
                CalendarWriteStatus.IDLE -> Hidden
                CalendarWriteStatus.SAVING -> Spinner
                CalendarWriteStatus.SAVED -> Saved
                CalendarWriteStatus.NOT_SAVED -> NotSaved
                CalendarWriteStatus.UNCONFIRMED -> Unconfirmed
            }
    }
}

/** Whether [this] settled without the change being known to have saved. */
internal val CalendarWriteStatus.failed: Boolean
    get() = CalendarWriteIndicator.of(this).offersRetry
