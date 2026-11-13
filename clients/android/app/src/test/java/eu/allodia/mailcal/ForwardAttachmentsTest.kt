// What a composer does with the files the core holds for it (docs/composer-layout.md).
//
// A forward's originals stop being special once they are staged: they are tiles among the ones a
// picked file lands in, and each tile's Remove goes through the core. A regression here is
// invisible at runtime in the worst way, since the message still sends, just without what it was
// forwarding.
package eu.allodia.mailcal

import android.text.format.Formatter
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.ShadowDialog
import uniffi.mailcal_bindings.ComposeKind
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.ComposerFileTile
import uniffi.mailcal_bindings.ComposerFiles
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.DraftStatus

/**
 * A stand-in for the core's list, answering as the core does: each file gets an id of its own, and
 * `changed` is whether the list differs from the one the composer opened holding.
 */
internal class FakeComposerFileList(opened: List<ComposerFileTile>) : ComposerFileList {
    private val openedWith = opened.map { it.id }
    var tiles = opened
        private set
    private var next = opened.size

    override fun current() = answer()

    override fun add(files: List<ComposerFileAttachment>): ComposerFiles {
        tiles = tiles + files.map { ComposerFileTile("file-${next++}", it.fileName, null, FREE) }
        return answer()
    }

    override fun remove(id: String): ComposerFiles {
        tiles = tiles.filterNot { it.id == id }
        return answer()
    }

    private fun answer() = ComposerFiles(tiles, tiles.map { it.id } != openedWith)
}

@RunWith(RobolectricTestRunner::class)
class ForwardAttachmentsTest {
    @get:Rule val rule = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private fun originals() = FakeComposerFileList(
        listOf(
            ComposerFileTile("file-0", "invoice.pdf", 2048u, FREE),
            ComposerFileTile("file-1", "terms.docx", null, FREE),
        ),
    )

    /** The tiles as the composer draws them: the core's last answer, redrawn from each call. */
    private fun tiles(list: ComposerFileList) {
        rule.setContent {
            var files by remember { mutableStateOf(checkNotNull(list.current())) }
            ComposerFileTiles(files) { tile -> list.remove(tile.id)?.let { files = it } }
        }
    }

    // The name is the core's answer, and the size is worded in the device's own units; a file the
    // core could not read has none to give.
    @Test
    fun `the originals files show as tiles with their size`() {
        tiles(originals())

        rule.onNodeWithText("invoice.pdf").assertIsDisplayed()
        rule.onNodeWithText(Formatter.formatShortFileSize(ctx(), 2048)).assertIsDisplayed()
        rule.onNodeWithText("terms.docx").assertIsDisplayed()
    }

    @Test
    fun `a file the user would rather not pass on is taken off through the core`() {
        val list = originals()
        tiles(list)

        // One Remove per tile, in the core's order, so the first belongs to the first file.
        rule.onAllNodesWithText(L10n.action_remove(ctx()))[0].performClick()
        rule.waitForIdle()

        assertEquals(listOf("terms.docx"), list.tiles.map { it.fileName })
        rule.onNodeWithText("invoice.pdf").assertDoesNotExist()
        assertTrue("taking an original off is a change", checkNotNull(list.current()).changed)
    }

    // The Remove is drawn from the core's state through the control components, so a removal the
    // core is running is held and spins (docs/waiting.md).
    @Test
    fun `a remove the core is running is held with its spinner`() {
        val running = ControlState(enabled = false, running = true)
        tiles(FakeComposerFileList(listOf(ComposerFileTile("file-0", "plan.pdf", 10u, running))))

        rule.onNodeWithTag("composer-file-remove").assertIsNotEnabled()
        rule.onNodeWithTag("composer-file-remove-running").assertIsDisplayed()
    }

    // Abandoning a forward loses nothing: its files are still in the mailbox.
    @Test
    fun `a forward nobody touched is not a draft for carrying the originals files`() {
        val left = forwardComposer()
        close()
        assertTrue("nothing was written, so nothing is saved", left.savedAndClosed.isEmpty())
        assertFalse(left.open)
    }

    // Taking one off is a decision about what goes out, so leaving keeps the composer in Drafts.
    @Test
    fun `taking a file off a forward makes it a draft`() {
        val left = forwardComposer()
        rule.onAllNodesWithText(L10n.action_remove(ctx()))[0].performClick()
        rule.waitForIdle()
        rule.onNodeWithText("invoice.pdf").assertDoesNotExist()

        close()
        answerEditor("""{"blocks":[]}""")
        assertEquals(listOf("c-fwd"), left.savedAndClosed)
        assertFalse(left.open)
    }

    @Test
    fun `whether the files changed is the core's answer`() {
        fun edited(filesChanged: Boolean) = composerHeadersEdited(
            to = "bob@test.local",
            initialTo = "bob@test.local",
            cc = "",
            initialCc = "",
            bcc = "",
            initialBcc = "",
            subject = "Fwd: Lunch",
            initialSubject = "Fwd: Lunch",
            filesChanged = filesChanged,
        )
        assertFalse(edited(filesChanged = false))
        assertTrue(edited(filesChanged = true))
    }

    private class Left {
        val savedAndClosed = mutableListOf<String>()
        var open = true
    }

    /** The real composer, over a forward's originals and a recorder of how it was left. */
    private fun forwardComposer(): Left {
        val left = Left()
        val opening = testOpening(kind = ComposeKind.FORWARD, composition = "c-fwd")
        val drafts = ComposerDrafts(
            save = { _, _ -> },
            saveAndClose = { composition, _ -> left.savedAndClosed += composition },
            discard = {},
            close = {},
            status = { DraftStatus.IDLE },
            isStored = { false },
            version = 0,
            idleSeconds = 30u,
        )
        val files = originals()
        rule.setContent {
            var shown by remember { mutableStateOf(true) }
            AppTheme {
                if (shown) {
                    RichComposeMessageDialog(
                        request = ComposeContext(opening, initialFrom = null),
                        files = files,
                        onDismiss = {
                            shown = false
                            left.open = false
                        },
                        onSubmitRich = { _ -> true },
                        accounts = emptyList(),
                        drafts = drafts,
                    )
                }
            }
        }
        rule.waitForIdle()
        rule.onNodeWithText("invoice.pdf").assertExists()
        return left
    }

    private fun close() {
        rule.onNodeWithContentDescription(L10n.action_close(ctx())).performClick()
        rule.waitForIdle()
    }

    /**
     * Answers the editor's pending `composerDocument()` read the way the page would: Robolectric's
     * WebView runs no script, it records the last one and waits.
     */
    private fun answerEditor(documentJson: String) {
        val editor = webViewIn(checkNotNull(ShadowDialog.getLatestDialog().window).decorView)
        val shadow = shadowOf(checkNotNull(editor) { "the composer holds no editor" })
        assertEquals("composerDocument()", shadow.lastEvaluatedJavascript)
        shadow.lastEvaluatedJavascriptCallback.onReceiveValue(JSONObject.quote(documentJson))
        rule.waitForIdle()
    }

    private fun webViewIn(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webViewIn(view.getChildAt(it)) }
        else -> null
    }
}
