// An event's video meetings: the decisions behind the detail's meeting section and the camera on
// every event row, kept out of Compose so the JVM suite can drive them (docs/conferencing.md). The
// twin of Apple's EventConferenceView helpers and Windows' EventMeeting.cs; the section itself is
// EventConferenceSection.kt.
//
// Which meetings exist, which link Join opens and which numbers are dialled is the core's
// (`EventDetail.conferences`): `joinUrl` and a dial-in's `uri` have already passed the link
// policy, so they are opened as they are.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.ConferenceState
import uniffi.mailcal_bindings.EventConference

internal object EventMeeting {
    /**
     * The meeting's heading: the service's own name, a brand the same in every language, or the
     * catalog's words for a video meeting ([videoMeeting]) when nothing names one.
     */
    fun title(conference: EventConference, videoMeeting: String): String =
        conference.service?.takeIf { it.isNotEmpty() } ?: videoMeeting

    /**
     * What the section says in place of Join while the server is still making the meeting
     * ([preparing]), or after it could not ([failed]); `null` for a meeting that is ready.
     */
    fun status(conference: EventConference, preparing: String, failed: String): String? =
        when (conference.state) {
            ConferenceState.READY -> null
            ConferenceState.PREPARING -> preparing
            ConferenceState.FAILED -> failed
        }

    /** Whether a row for [value] is drawn: one with no value is left out. */
    fun shows(value: String?): Boolean = !value.isNullOrEmpty()

    /**
     * An event's spoken label with "video meeting" ([videoMeeting]) after it when it has one: the
     * camera is drawn on a canvas or hidden from TalkBack, so the label says what it shows.
     */
    fun spoken(label: String, videoMeeting: String, hasConference: Boolean): String =
        if (hasConference) "$label, $videoMeeting" else label
}
