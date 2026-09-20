// Keeping the composer's message on the server, as the composer drives it (docs/drafts.md).
//
// What is Android's here, and what this covers, is which of the core's verbs each way out of the
// composer reaches. The distinction that earns a test is that only ONE of them takes the stored
// copy off the server: dismissing a composer leaves the draft in Drafts, and Discard removes it.
// Getting that backwards deletes a draft the user was only closing, or leaves one they threw away,
// and neither failure says anything on screen.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.DraftStatus

@RunWith(RobolectricTestRunner::class)
class ComposerDraftTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    /** Every composition the composer named, in the order it named them. */
    private class Recorder {
        val saved = mutableListOf<String>()
        val discarded = mutableListOf<String>()
        val closed = mutableListOf<String>()

        fun drafts() = ComposerDrafts(
            save = { composition, _ -> saved += composition },
            discard = { discarded += it },
            close = { closed += it },
            status = { DraftStatus.IDLE },
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
    private fun composer(recorder: Recorder, composition: String? = null) {
        compose.setContent {
            var open by remember { mutableStateOf(true) }
            AppTheme {
                if (open) {
                    RichComposeMessageDialog(
                        mode = RichComposeMode.New,
                        onDismiss = { open = false },
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

    private fun close() {
        compose.onNodeWithContentDescription(L10n.action_cancel(ctx())).performClick()
        compose.waitForIdle()
    }

    @Test
    fun discarding_takes_the_stored_copy_away() {
        val recorder = Recorder()
        composer(recorder)
        compose.onNodeWithText(L10n.compose_subject(ctx())).performTextInput("Half a sentence")
        close()
        compose.onNodeWithText(L10n.action_discard(ctx())).performClick()
        compose.waitForIdle()
        assertEquals(1, recorder.discarded.size)
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
