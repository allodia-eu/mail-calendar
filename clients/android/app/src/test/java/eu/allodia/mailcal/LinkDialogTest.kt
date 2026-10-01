package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ComposerHostRequest
import uniffi.mailcal_bindings.ComposerLinkAnswer

// The link dialog and the channel that opens it. The core's parser and address rule are stood in
// for, because nothing in this suite loads the native library: what is Android's is that a request
// reaches the dialog on the main thread, that Apply follows the core's answer, and that each way
// out of the dialog answers the editor once.
@RunWith(RobolectricTestRunner::class)
class LinkDialogTest {
    @get:Rule val compose = createComposeRule()
    private val ctx = ApplicationProvider.getApplicationContext<Context>()

    // The core's rule, enough of it for these tests: an address with a dot is a link.
    private fun fakeAddress(typed: String): String? = typed.takeIf { "." in it }?.let { "https://$it" }

    private fun show(request: LinkDialogRequest): MutableList<ComposerLinkAnswer> {
        val answers = mutableListOf<ComposerLinkAnswer>()
        compose.setContent { LinkDialog(request, linkAddress = ::fakeAddress) { answers += it } }
        return answers
    }

    @Test
    fun applyFollowsTheCoresAnswerAndSendsWhatWasTyped() {
        val answers = show(LinkDialogRequest(id = 1u, text = "the agenda", address = "", removable = false))

        compose.onNodeWithText(L10n.editor_link_dialog_insert(ctx)).assertExists()
        compose.onNodeWithText(L10n.editor_link_apply(ctx)).assertIsNotEnabled()
        compose.onNodeWithTag("link-address").performTextReplacement("not a link")
        compose.onNodeWithText(L10n.editor_link_invalid(ctx)).assertExists()
        compose.onNodeWithText(L10n.editor_link_apply(ctx)).assertIsNotEnabled()

        compose.onNodeWithTag("link-address").performTextReplacement("example.com")
        compose.onNodeWithText(L10n.editor_link_apply(ctx)).assertIsEnabled().performClick()

        assertEquals(listOf(ComposerLinkAnswer.Apply("the agenda", "example.com")), answers)
    }

    @Test
    fun removeIsOfferedOnlyOnALink() {
        val answers = show(LinkDialogRequest(id = 2u, text = "Ann", address = "a@example.com", removable = true))

        compose.onNodeWithText(L10n.editor_link_dialog_edit(ctx)).assertExists()
        compose.onNodeWithText(L10n.editor_link_remove(ctx)).performClick()
        assertEquals(listOf<ComposerLinkAnswer>(ComposerLinkAnswer.Remove), answers)
    }

    @Test
    fun cancelAnswersCancel() {
        val answers = show(LinkDialogRequest(id = 3u, text = "", address = "", removable = false))

        compose.onNodeWithText(L10n.editor_link_remove(ctx)).assertDoesNotExist()
        compose.onNodeWithText(L10n.action_cancel(ctx)).performClick()
        assertEquals(listOf<ComposerLinkAnswer>(ComposerLinkAnswer.Cancel), answers)
    }

    @Test
    fun aRequestReachesTheDialogThroughThePosterAndAnythingElseIsDropped() {
        val posted = mutableListOf<Runnable>()
        val opened = mutableListOf<LinkDialogRequest>()
        val channel = ComposerHostChannel(
            onLink = { opened += it },
            parse = { message ->
                if (message == "link") ComposerHostRequest.Link(7u, "docs", "", false) else null
            },
            post = { posted += it },
        )

        channel.postMessage("something else")
        assertTrue(posted.isEmpty())

        channel.postMessage("link")
        // Not delivered on the bridge thread: only once the poster runs it.
        assertTrue(opened.isEmpty())
        posted.single().run()
        assertEquals(listOf(LinkDialogRequest(7u, "docs", "", false)), opened)
    }
}
