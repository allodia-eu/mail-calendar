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
 * `Warning` is deliberately *not* "your change was rejected": the core has already confirmed the write
 * reached the server (see `CalendarWriteStatus`), and a full refresh reconciles the local view, so the
 * warning offers a retry (a `RefreshCalendar`), it does not report a lost edit.
 */
internal enum class CalendarWriteIndicator {
    /** Nothing to show. */
    Hidden,

    /** A write is settling, a small spinner. */
    Spinner,

    /** The write settled and the local view holds the server's copy, a brief check. */
    Saved,

    /** The write could not be confirmed, a warning the user can tap to retry. */
    Warning,
    ;

    /** Whether tapping the indicator should trigger a retry (a `RefreshCalendar`). */
    val offersRetry: Boolean
        get() = this == Warning

    companion object {
        /** Maps a core [CalendarWriteStatus] to what the header shows. Total and pure. */
        fun of(status: CalendarWriteStatus): CalendarWriteIndicator =
            when (status) {
                CalendarWriteStatus.IDLE -> Hidden
                CalendarWriteStatus.SAVING -> Spinner
                CalendarWriteStatus.SAVED -> Saved
                CalendarWriteStatus.FAILED -> Warning
            }
    }
}
