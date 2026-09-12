// What a forward composer does with the files the original carries.
//
// The point of staging them is that they stop being special: they land in the same list a picked
// file lands in, and the remove button beside each one works. A regression here is invisible at
// runtime in the worst way, since the message still sends, just without what it was forwarding.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.ComposerFileAttachment

@RunWith(RobolectricTestRunner::class)
class ForwardAttachmentsTest {
    @get:Rule val rule = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val staged = listOf(
        ComposerFileAttachment("/cache/0-invoice.pdf", "invoice.pdf", "application/pdf"),
        ComposerFileAttachment("/cache/1-terms.docx", "terms.docx", "application/msword"),
    )

    // The name and media type are the core's answer and are not re-derived from the path, which
    // is a uniquified temporary the recipient must never see.
    @Test
    fun `a staged file keeps the name its sender gave it`() {
        val row = seededComposerFile(staged[0])

        assertEquals("invoice.pdf", row.fileName)
        assertEquals("application/pdf", row.mediaType)
        assertEquals("/cache/0-invoice.pdf", row.path)
        assertEquals("invoice.pdf", row.composerFile.fileName)
    }

    @Test
    fun `the originals files show as ordinary attachments`() {
        rule.setContent {
            ComposerAttachmentList(attachments = staged.map(::seededComposerFile), onRemove = {})
        }

        rule.onNodeWithText("invoice.pdf").assertIsDisplayed()
        rule.onNodeWithText("terms.docx").assertIsDisplayed()
    }

    @Test
    fun `a file the user would rather not pass on can be removed`() {
        var removed: PickedComposerFile? = null
        val rows = staged.map(::seededComposerFile)
        rule.setContent {
            ComposerAttachmentList(attachments = rows, onRemove = { removed = it })
        }

        // One Remove button per row, in row order, so the first belongs to the first file.
        rule.onAllNodesWithText(L10n.action_remove(ctx()))[0].performClick()

        assertEquals("invoice.pdf", removed?.fileName)
    }

    // Staging failing and staging finding nothing are different answers, and the seed keeps them
    // apart: an empty list with no failure says the message had nothing attached, which is the one
    // claim a failure must not be allowed to make.
    @Test
    fun `an empty seed is not the same as a failed one`() {
        assertTrue(ForwardSeed().files.isEmpty())
        assertFalse(ForwardSeed().failed)
        assertTrue(ForwardSeed(failed = true).failed)
    }

    // Abandoning a forward loses nothing: the files are still in the mailbox. Measured against
    // zero instead, every forward the user thought better of would stop them with a prompt about
    // work they never did.
    @Test
    fun `a forward nobody touched is not a draft for carrying the originals files`() {
        assertFalse(untouchedForward(attachments = 2, initialAttachments = 2))
        // Taking one off is a decision about what goes out, and it would be lost.
        assertTrue(untouchedForward(attachments = 1, initialAttachments = 2))
        // A file the user added themselves is work, as it is on every other kind.
        assertTrue(untouchedForward(attachments = 3, initialAttachments = 2))
    }

    private fun untouchedForward(attachments: Int, initialAttachments: Int) =
        composerHeadersEdited(
            to = "bob@test.local",
            initialTo = "bob@test.local",
            cc = "",
            initialCc = "",
            bcc = "",
            initialBcc = "",
            subject = "Fwd: Lunch",
            initialSubject = "Fwd: Lunch",
            attachments = attachments,
            initialAttachments = initialAttachments,
        )
}
