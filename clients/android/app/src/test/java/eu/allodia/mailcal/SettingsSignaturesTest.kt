// Settings → Signatures as the core draws it (docs/signatures.md, "Changing the library";
// docs/waiting.md). The screen keeps no rule of its own: every control is the page's, a change is a
// `SignatureIntent`, and the editor closes or stays open on the outcome the page says for the
// signature it edits. Driven through the real composables with Compose's test rule.
package eu.allodia.mailcal

import android.text.format.Formatter
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.ShadowDialog
import uniffi.mailcal_bindings.SignatureChangeKind
import uniffi.mailcal_bindings.SignatureImageRefusal
import uniffi.mailcal_bindings.SignatureIntent
import uniffi.mailcal_bindings.SignatureOutcome
import uniffi.mailcal_bindings.SignatureRefusal
import uniffi.mailcal_bindings.SignatureResult
import uniffi.mailcal_bindings.SignatureSlotKind

private val WORK = signatureEntry("sig-work", "Work")
private val PERSONAL = signatureEntry("sig-home", "Personal")

private fun saved(serial: Int, id: String, result: SignatureResult = SignatureResult.Saved) =
    SignatureOutcome(serial = serial.toULong(), id = id, kind = SignatureChangeKind.SAVE, result = result)

@RunWith(RobolectricTestRunner::class)
class SettingsSignaturesTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<SignatureIntent>()
    private val bodiesAsked = mutableListOf<String>()

    /** The page as the core answers it; a change here is a page read after a signal. */
    private var page by mutableStateOf(signaturesPage())

    private fun show(library: Boolean = true) {
        compose.setContent {
            AppTheme {
                val ui = testSignaturesUi(
                    page = page,
                    bodyHtml = { id ->
                        bodiesAsked += id
                        "<p>$id</p>"
                    },
                    onChange = { sent += it },
                )
                Column {
                    if (library) SignatureLibraryCard(ui) else AccountSignatureDefaultsCard(ui)
                }
            }
        }
        compose.waitForIdle()
    }

    /** Answers the editor's pending `signatureBody()` read the way the bundle would. */
    private fun answerBody(html: String, plain: String) {
        val editor = webViewIn(checkNotNull(ShadowDialog.getLatestDialog().window).decorView)
        val shadow = shadowOf(checkNotNull(editor) { "the signature editor holds no editor" })
        assertEquals("window.signatureBody()", shadow.lastEvaluatedJavascript)
        val body = JSONObject().put("body_html", html).put("body_plain", plain).toString()
        shadow.lastEvaluatedJavascriptCallback.onReceiveValue(JSONObject.quote(body))
        compose.waitForIdle()
    }

    private fun webViewIn(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webViewIn(view.getChildAt(it)) }
        else -> null
    }

    private fun editorIsOpen() = compose.onAllNodes(hasText(L10n.settings_signatures_name_label(ctx())))
        .fetchSemanticsNodes().isNotEmpty()

    @Test
    fun each_control_is_drawn_from_the_page() {
        page = signaturesPage(
            signatures = listOf(signatureEntry("sig-work", "Work", edit = HELD, delete = SIGNATURE_RUNNING), PERSONAL),
            add = HELD,
        )
        show()

        compose.onNodeWithText("Work").assertIsDisplayed()
        compose.onNodeWithText("Personal").assertIsDisplayed()
        compose.onNodeWithTag(signatureDeleteTag("sig-work") + "-running").assertExists()
        compose.onNodeWithTag(signatureDeleteTag("sig-work")).assertIsNotEnabled()
        compose.onNodeWithTag(signatureEditTag("sig-work")).assertIsNotEnabled()
        compose.onNodeWithTag(signatureEditTag("sig-home")).assertIsEnabled()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).assertIsNotEnabled()
    }

    @Test
    fun delete_asks_first_and_then_sends_the_deletion() {
        page = signaturesPage(signatures = listOf(WORK, PERSONAL))
        show()

        compose.onNodeWithTag(signatureDeleteTag("sig-work")).performClick()
        compose.onNodeWithText(L10n.action_cancel(ctx())).performClick()
        assertEquals("cancelling sends nothing", emptyList<SignatureIntent>(), sent)

        compose.onNodeWithTag(signatureDeleteTag("sig-work")).performClick()
        compose.onNodeWithTag("$SIGNATURE_DELETE_CONFIRM_TAG-confirm").performClick()
        assertEquals(listOf(SignatureIntent.Delete("sig-work")), sent)
    }

    @Test
    fun a_slot_shows_its_selection_and_a_pick_assigns_that_slot() {
        page = signaturesPage(
            signatures = listOf(WORK, PERSONAL),
            accounts = listOf(accountSignatures("acct", newMessage = "sig-work", replyForward = "sig-home")),
        )
        show(library = false)
        val newMessage = signatureSlotTag("acct", SignatureSlotKind.NEW_MESSAGE)
        val replyForward = signatureSlotTag("acct", SignatureSlotKind.REPLY_FORWARD)
        compose.onNodeWithTag(newMessage).assert(hasText("Work"))
        compose.onNodeWithTag(replyForward).assert(hasText("Personal"))

        compose.onNodeWithTag(newMessage).performClick()
        compose.onNodeWithText(L10n.settings_signatures_none(ctx())).performClick()
        assertEquals(listOf(SignatureIntent.Assign("acct", SignatureSlotKind.NEW_MESSAGE, null)), sent)
    }

    @Test
    fun a_held_slot_cannot_be_opened() {
        page = signaturesPage(signatures = listOf(WORK), accounts = listOf(accountSignatures("acct", control = HELD)))
        show(library = false)
        compose.onNodeWithTag(signatureSlotTag("acct", SignatureSlotKind.NEW_MESSAGE))
            .assert(hasText(L10n.settings_signatures_none(ctx())))
            .assertIsNotEnabled()
    }

    @Test
    fun a_new_signature_saves_under_the_new_id_with_the_name_as_typed() {
        show()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).performClick()
        compose.onNodeWithTag(SIGNATURE_SAVE_TAG).assertIsEnabled()

        compose.onNodeWithTag(SIGNATURE_NAME_TAG).performTextClearance()
        compose.onNodeWithTag(SIGNATURE_SAVE_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(SIGNATURE_NAME_TAG).performTextInput("  Work  ")
        compose.onNodeWithTag(SIGNATURE_SAVE_TAG).assertIsEnabled().performClick()
        answerBody("<p>Alice</p>", "Alice")

        assertEquals(listOf(SignatureIntent.Save("sig-new", "  Work  ", "<p>Alice</p>", "Alice")), sent)
        assertTrue("the editor waits for the outcome", editorIsOpen())
    }

    @Test
    fun editing_saves_under_the_signature_s_own_id() {
        page = signaturesPage(signatures = listOf(WORK))
        show()
        compose.onNodeWithTag(signatureEditTag("sig-work")).performClick()
        assertEquals(listOf("sig-work"), bodiesAsked)

        compose.onNodeWithTag(SIGNATURE_SAVE_TAG).performClick()
        answerBody("<p>sig-work</p>", "sig-work")
        assertEquals(listOf(SignatureIntent.Save("sig-work", "Work", "<p>sig-work</p>", "sig-work")), sent)
    }

    @Test
    fun save_runs_while_the_page_says_a_save_runs() {
        show()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).performClick()
        page = page.copy(save = SIGNATURE_RUNNING)
        compose.waitForIdle()

        compose.onNodeWithTag("$SIGNATURE_SAVE_TAG-running").assertExists()
        compose.onNodeWithTag(SIGNATURE_SAVE_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(SIGNATURE_NAME_TAG).assertIsNotEnabled()
    }

    @Test
    fun only_a_newer_outcome_for_this_signature_closes_the_editor() {
        page = signaturesPage(outcome = saved(3, "sig-new"))
        show()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).performClick()
        assertTrue("an outcome in force at open is not this editor's", editorIsOpen())

        page = page.copy(outcome = saved(4, "sig-other"))
        compose.waitForIdle()
        assertTrue("another signature's outcome is not this editor's", editorIsOpen())

        page = page.copy(outcome = saved(5, "sig-new"))
        compose.waitForIdle()
        assertTrue("a save that was made closes it", !editorIsOpen())
    }

    @Test
    fun a_save_that_could_not_be_written_closes_the_editor() {
        show()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).performClick()
        page = page.copy(outcome = saved(1, "sig-new", SignatureResult.NotWritten))
        compose.waitForIdle()
        assertTrue(!editorIsOpen())
    }

    @Test
    fun a_refused_save_keeps_the_editor_open_and_says_why() {
        show()
        compose.onNodeWithTag(SIGNATURE_ADD_TAG).performClick()
        page = page.copy(outcome = saved(1, "sig-new", SignatureResult.Refused(SignatureRefusal.Gone)))
        compose.waitForIdle()

        assertTrue(editorIsOpen())
        compose.onNodeWithText(L10n.settings_signatures_gone(ctx())).assertIsDisplayed()
    }

    @Test
    fun each_refusal_is_worded_from_the_catalog() {
        assertEquals(L10n.settings_signatures_name_required(ctx()), signatureRefusalText(ctx(), SignatureRefusal.BlankName))
        assertEquals(
            L10n.settings_signatures_image_format(ctx()),
            signatureRefusalText(ctx(), SignatureRefusal.Image(SignatureImageRefusal.Format)),
        )
    }

    /** The limit said is the one the core refused with, not a figure of the client's own. */
    @Test
    fun a_picture_too_large_names_the_core_s_limit() {
        val small = signatureImageRefusalText(ctx(), SignatureImageRefusal.TooLarge(1000uL))
        val large = signatureImageRefusalText(ctx(), SignatureImageRefusal.TooLarge(2_000_000uL))

        assertEquals(
            L10n.settings_signatures_image_too_large(ctx(), limit = Formatter.formatShortFileSize(ctx(), 1000)),
            small,
        )
        assertNotEquals(small, large)
    }
}
