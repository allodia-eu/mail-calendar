// Keeping the composer's message on the server, as the composer drives it (docs/drafts.md).
//
// What is Android's here, and what this covers, is which of the core's verbs each way out of the
// composer reaches. The distinction that earns a test is that only ONE of them takes the stored
// copy off the server: leaving a composer keeps the draft in Drafts, unasked, and Discard removes
// it. Getting that backwards deletes a draft the user was only leaving, or leaves one they threw
// away, and neither failure says anything on screen.
package eu.allodia.mailcal

import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.ShadowDialog
import uniffi.mailcal_bindings.DraftStatus

@RunWith(RobolectricTestRunner::class)
class ComposerDraftTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    /** Every composition the composer named, in the order it named them. */
    private class Recorder(val stored: Boolean = false) {
        val saved = mutableListOf<String>()
        val savedAndClosed = mutableListOf<String>()
        val discarded = mutableListOf<String>()
        val closed = mutableListOf<String>()

        fun drafts() = ComposerDrafts(
            save = { composition, _ -> saved += composition },
            saveAndClose = { composition, _ -> savedAndClosed += composition },
            discard = { discarded += it },
            close = { closed += it },
            status = { DraftStatus.IDLE },
            isStored = { stored },
            version = 0,
            // The core's value; read here as a literal, because the JVM suite renders
            // this composer without the cdylib loaded.
            idleSeconds = 30u,
        )
    }

    /**
     * Renders the real composer over `recorder`'s verbs.
     *
     * Dismissing actually takes it off screen, rather than only recording that it was asked to:
     * forgetting the composition is disposal work, so a composer left composed would never do it,
     * and the test would pass over a client that never forgot anything.
     */
    private var open = true

    private fun composer(recorder: Recorder, composition: String? = null) {
        compose.setContent {
            var shown by remember { mutableStateOf(true) }
            AppTheme {
                if (shown) {
                    RichComposeMessageDialog(
                        mode = RichComposeMode.New,
                        onDismiss = {
                            shown = false
                            open = false
                        },
                        onSubmitRich = { _ -> true },
                        accounts = emptyList(),
                        drafts = recorder.drafts(),
                        composition = composition,
                    )
                }
            }
        }
        compose.waitForIdle()
    }

    /** Leaves the composer by its ✕, the path the system back takes too (see ComposerDiscardTest). */
    private fun close() {
        compose.onNodeWithContentDescription(L10n.action_close(ctx())).performClick()
        compose.waitForIdle()
    }

    private fun pressDiscard() {
        compose.onNodeWithContentDescription(L10n.action_discard(ctx())).performClick()
        compose.waitForIdle()
    }

    /**
     * Answers the editor's pending `composerDocument()` read the way the page would.
     *
     * Robolectric's WebView runs no script: it records the last one and its callback and waits, so
     * a test that wants the composer to have read its document answers on the page's behalf.
     */
    private fun answerEditor(documentJson: String) {
        val editor = webViewIn(checkNotNull(ShadowDialog.getLatestDialog().window).decorView)
        val shadow = shadowOf(checkNotNull(editor) { "the composer holds no editor" })
        assertEquals("composerDocument()", shadow.lastEvaluatedJavascript)
        shadow.lastEvaluatedJavascriptCallback.onReceiveValue(JSONObject.quote(documentJson))
        compose.waitForIdle()
    }

    private fun webViewIn(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webViewIn(view.getChildAt(it)) }
        else -> null
    }

    @Test
    fun leaving_a_written_composer_keeps_it_in_drafts_without_asking() {
        val recorder = Recorder()
        composer(recorder, composition = "c-1")
        compose.onNodeWithText(L10n.compose_subject(ctx())).performTextInput("Half a sentence")

        close()
        answerEditor("""{"blocks":[]}""")

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertEquals(listOf("c-1"), recorder.savedAndClosed)
        assertTrue("leaving is not discarding", recorder.discarded.isEmpty())
        // The save-and-close already finished with the composition; closing it again from
        // disposal would be a second answer to the same question.
        assertTrue(recorder.closed.isEmpty())
        assertFalse("the composer closed", open)
    }

    @Test
    fun discard_asks_when_something_was_written() {
        val recorder = Recorder()
        composer(recorder)
        compose.onNodeWithText(L10n.compose_subject(ctx())).performTextInput("Half a sentence")

        pressDiscard()
        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertExists()
        compose.onNodeWithText(L10n.action_discard(ctx())).performClick()
        compose.waitForIdle()

        assertEquals(1, recorder.discarded.size)
        assertTrue(recorder.closed.isEmpty())
        assertFalse(open)
    }

    @Test
    fun discard_asks_about_an_untouched_composer_whose_copy_is_in_drafts() {
        // A resumed draft opens untouched, and Discard would still delete it from Drafts.
        val recorder = Recorder(stored = true)
        composer(recorder, composition = "c-1")

        pressDiscard()
        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertExists()
        compose.onNodeWithText(L10n.action_keep_editing(ctx())).performClick()
        compose.waitForIdle()

        assertTrue("keep editing removes nothing", recorder.discarded.isEmpty())
        assertTrue(open)
    }

    @Test
    fun discard_with_nothing_to_lose_closes_at_once() {
        val recorder = Recorder()
        composer(recorder, composition = "c-1")

        pressDiscard()

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertEquals(listOf("c-1"), recorder.discarded)
        assertTrue(recorder.closed.isEmpty())
        assertFalse(open)
    }

    @Test
    fun closing_a_composer_nobody_edited_leaves_the_draft_where_it_is() {
        // The case that makes this more than symmetry: a resumed draft opens clean, so closing it
        // without typing is the ordinary way of looking at a draft and putting it back. Discarding
        // there would delete the message the user had only opened.
        val recorder = Recorder()
        composer(recorder, composition = "c-1")
        close()
        assertTrue(recorder.discarded.isEmpty())
        assertTrue("nothing was written, so nothing is saved", recorder.savedAndClosed.isEmpty())
    }

    @Test
    fun a_resumed_composer_saves_under_the_composition_the_core_gave_it() {
        // The core has already joined that id to the copy on the server. A composer that minted
        // one of its own would store a second draft beside the one it is showing, and neither
        // would supersede the other.
        val recorder = Recorder()
        composer(recorder, composition = "c-1")
        close()
        assertEquals(listOf("c-1"), recorder.closed)
    }

    @Test
    fun the_hint_says_nothing_until_something_has_been_saved() {
        // A hint, never a gate, and a composer that has saved nothing has nothing to report.
        assertNull(composerDraftHint(DraftStatus.IDLE, ctx()))
        assertEquals(L10n.compose_draft_saved(ctx()), composerDraftHint(DraftStatus.SAVED, ctx()))
        assertEquals(L10n.compose_draft_queued(ctx()), composerDraftHint(DraftStatus.QUEUED, ctx()))
    }

    @Test
    fun the_editor_is_sampled_several_times_within_one_idle_interval() {
        // The sample is what decides how late a draft can be: too coarse and a composer that has
        // gone quiet waits nearly twice the interval before its words reach the server.
        assertEquals(10_000L, draftSampleMillis(30u))
        // Never zero, whatever the core's interval is: a sampler on a zero delay is a spin.
        assertTrue(draftSampleMillis(1u) > 0L)
        assertTrue(draftSampleMillis(0u) > 0L)
    }
}
