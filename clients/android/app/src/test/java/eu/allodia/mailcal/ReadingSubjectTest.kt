// The title over an open message (ReadingScreen.kt, docs/background-sync.md).
//
// A notification tap opens the message it names straight away, and the list may not hold that
// message yet, so the header can open with no row behind it. The core's snapshot carries the
// subject for exactly that case; these pin that the screen takes it, and only for its own message.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.ReadingSnapshot

class ReadingSubjectTest {
    private fun opened(subject: String) = OpenedMessage(
        account = "acct-1",
        key = "m1",
        subject = subject,
        from = "",
        avatar = stubAvatar(),
        date = "",
    )

    private fun snapshot(key: String, subject: String) = ReadingSnapshot(
        key = key,
        subject = subject,
        from = "Ada Lovelace <ada@example.test>",
        avatar = stubAvatar(),
        to = "",
        cc = "",
        bcc = "",
        html = null,
        plain = "Body",
        hasRemoteImages = false,
        loadError = false,
        attachments = emptyList(),
        invitation = null,
        pending = false,
    )

    @Test
    fun anOpenWithNoRowTakesItsTitleFromTheSnapshot() {
        assertEquals("Quarterly report", displayedSubject(opened(""), snapshot("m1", "Quarterly report")))
    }

    @Test
    fun theRowsSubjectStandsUntilTheSnapshotArrives() {
        assertEquals("Quarterly report", displayedSubject(opened("Quarterly report"), null))
    }

    @Test
    fun aSnapshotForAnotherMessageIsNotThisOnesTitle() {
        // A stale snapshot from the message opened before this one.
        assertEquals("", displayedSubject(opened(""), snapshot("m0", "Budget review")))
    }
}
