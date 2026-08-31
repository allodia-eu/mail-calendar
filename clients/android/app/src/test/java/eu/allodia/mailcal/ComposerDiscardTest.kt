// The unsent-draft guard's dirtiness rule, which Android inherited when the back
// button made "throw the draft away" a one-swipe accident.
//
// The rule is the desktops': compare against what the composer OPENED with, not against empty.
// That distinction is the whole test, a reply arrives with its To (and a reply-all with its Cc)
// pre-filled by the core, so "non-empty" would prompt on every reply nobody had typed into, and a
// prompt that fires when there is nothing to lose is the one failure that teaches users to dismiss
// it without reading.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class ComposerDiscardTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    /** A reply as the core hands it over: To pre-filled, nothing typed. */
    private fun untouchedReply(
        to: String = "bob@test.local",
        cc: String = "",
        bcc: String = "",
        subject: String = "",
        attachments: Int = 0,
    ) = composerHeadersEdited(
        to = to,
        initialTo = "bob@test.local",
        cc = cc,
        initialCc = "",
        bcc = bcc,
        initialBcc = "",
        subject = subject,
        initialSubject = "",
        attachments = attachments,
    )

    /** A mail link as the core decoded it: every field may arrive pre-filled, none of it typed. */
    private fun untouchedMailLink(
        to: String = "bob@test.local",
        cc: String = "carol@test.local",
        bcc: String = "dave@test.local",
        subject: String = "Lunch on Friday",
        attachments: Int = 0,
    ) = composerHeadersEdited(
        to = to,
        initialTo = "bob@test.local",
        cc = cc,
        initialCc = "carol@test.local",
        bcc = bcc,
        initialBcc = "dave@test.local",
        subject = subject,
        initialSubject = "Lunch on Friday",
        attachments = attachments,
    )

    @Test
    fun a_reply_nobody_typed_into_is_not_a_draft() {
        assertFalse(untouchedReply())
    }

    @Test
    fun a_reply_all_keeps_its_prefilled_cc_clean_too() {
        assertFalse(
            composerHeadersEdited(
                to = "bob@test.local",
                initialTo = "bob@test.local",
                cc = "carol@test.local",
                initialCc = "carol@test.local",
                bcc = "",
                initialBcc = "",
                subject = "",
                initialSubject = "",
                attachments = 0,
            ),
        )
    }

    @Test
    fun a_new_message_opens_clean() {
        assertFalse(
            composerHeadersEdited(
                to = "",
                initialTo = "",
                cc = "",
                initialCc = "",
                bcc = "",
                initialBcc = "",
                subject = "",
                initialSubject = "",
                attachments = 0,
            ),
        )
    }

    @Test
    fun adding_a_recipient_to_a_reply_makes_it_a_draft() {
        assertTrue(untouchedReply(to = "bob@test.local, carol@test.local"))
    }

    /** Removing the pre-filled recipient counts too, that is an edit, and losing it would sting. */
    @Test
    fun clearing_a_replys_recipient_makes_it_a_draft() {
        assertTrue(untouchedReply(to = ""))
    }

    /**
     * A mail link fills in what the page asked for, up to all four fields, and none of it is the
     * user's work. Held against empty instead of against what the link supplied, every `mailto:`
     * would open already counting as a draft and ask before closing something nobody typed a word
     * into. Bcc and Subject are the two that make this a test rather than a note: they are the
     * fields a reply never pre-fills, so the guard only ever saw them empty before mail links.
     */
    @Test
    fun a_mail_link_that_filled_every_field_is_not_a_draft() {
        assertFalse(untouchedMailLink())
    }

    @Test
    fun editing_what_a_mail_link_suggested_makes_it_a_draft() {
        assertTrue(untouchedMailLink(subject = "Lunch on Saturday"))
        assertTrue(untouchedMailLink(bcc = ""))
        assertTrue(untouchedMailLink(cc = "carol@test.local, erin@test.local"))
    }

    @Test
    fun a_typed_cc_bcc_subject_or_attachment_each_make_it_a_draft() {
        assertTrue(untouchedReply(cc = "carol@test.local"))
        assertTrue(untouchedReply(bcc = "dave@test.local"))
        assertTrue(untouchedReply(subject = "Re: lunch"))
        assertTrue(untouchedReply(attachments = 1))
    }

    /**
     * Typing and then deleting lands back on the opening values and reads as clean, which is
     * true. There is nothing left to lose, so there is nothing to interrupt anyone about.
     */
    @Test
    fun typing_then_undoing_it_is_clean_again() {
        assertFalse(untouchedReply(subject = ""))
        assertFalse(untouchedReply(to = "bob@test.local"))
    }

    // ---- The wiring: the prompt actually reaches the screen ----------------------------------

    /** Renders the real composer, recording whether it actually closed. */
    private fun composer(
        closed: MutableList<Unit>,
        mode: RichComposeMode = RichComposeMode.New,
        initialTo: String = "",
        initialCc: String = "",
        initialBcc: String = "",
        initialSubject: String = "",
    ) {
        compose.setContent {
            AppTheme {
                RichComposeMessageDialog(
                    mode = mode,
                    onDismiss = { closed += Unit },
                    onSubmitRich = { _, _, _, _, _ -> true },
                    accounts = emptyList(),
                    initialTo = initialTo,
                    initialCc = initialCc,
                    initialBcc = initialBcc,
                    initialSubject = initialSubject,
                )
            }
        }
        compose.waitForIdle()
    }

    /**
     * Leaves the composer the way the ✕ does, which is also the way back does.
     *
     * The composer is a `Dialog`, so it owns a window with its own `OnBackPressedDispatcher`, and
     * back reaches it as `onDismissRequest` rather than through the activity's dispatcher this
     * suite drives. Both are wired to the SAME lambda (`requestDismiss`), so pressing ✕ here
     * exercises exactly the path back takes; that back does reach it is Compose `Dialog` behaviour,
     * confirmed on a device.
     */
    private fun closeComposer() {
        compose.onNodeWithContentDescription(L10n.action_cancel(ctx())).performClick()
        compose.waitForIdle()
    }

    /**
     * The reason this guard reached Android at all: back is one edge swipe, and it used to throw a
     * half-written message away without a word. Now it asks, the same prompt macOS and Windows
     * raise.
     */
    @Test
    fun leaving_the_composer_asks_before_discarding_written_work() {
        val closed = mutableListOf<Unit>()
        composer(closed)
        compose.onNodeWithText(L10n.compose_subject(ctx())).performTextInput("Lunch on Friday")

        closeComposer()

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertExists()
        assertTrue("nothing may close while the question is on screen", closed.isEmpty())

        // Keep editing puts the user back where they were, with the draft intact.
        compose.onNodeWithText(L10n.action_keep_editing(ctx())).performClick()
        compose.waitForIdle()
        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertTrue(closed.isEmpty())
        compose.onNodeWithText("Lunch on Friday").assertExists()
    }

    @Test
    fun discarding_from_the_prompt_closes_the_composer() {
        val closed = mutableListOf<Unit>()
        composer(closed)
        compose.onNodeWithText(L10n.compose_subject(ctx())).performTextInput("Lunch on Friday")
        closeComposer()

        compose.onNodeWithText(L10n.action_discard(ctx())).performClick()
        compose.waitForIdle()

        assertEquals(1, closed.size)
    }

    /**
     * An untouched composer closes silently. A prompt that fires when there is nothing to lose is
     * the one failure that teaches people to dismiss it without reading.
     */
    @Test
    fun leaving_an_untouched_composer_just_closes_it() {
        val closed = mutableListOf<Unit>()
        composer(closed)

        closeComposer()

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertEquals(1, closed.size)
    }

    /**
     * The same, for a reply-all, which is the one whose opening values are *rewritten* before they
     * reach the fields, so that every pre-filled address renders as a pill rather than the last one
     * arriving as half-typed text. Compare the field against the raw argument instead of against
     * that rewritten value and every single reply-all counts as edited the moment it opens: a
     * prompt on the way out of a message nobody touched.
     */
    @Test
    fun leaving_an_untouched_reply_all_just_closes_it_too() {
        val closed = mutableListOf<Unit>()
        composer(
            closed,
            mode = RichComposeMode.ReplyAll,
            initialTo = "bob@test.local, carol@test.local",
            initialCc = "dave@test.local",
        )

        closeComposer()

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertEquals(1, closed.size)
    }

    /**
     * And for a composer a mail link opened. The same rule as the reply-all above, reaching the two
     * fields only a link pre-fills: closing a `mailto:` composer nobody typed into must not ask.
     */
    @Test
    fun leaving_an_untouched_mail_link_composer_just_closes_it_too() {
        val closed = mutableListOf<Unit>()
        composer(
            closed,
            initialTo = "bob@test.local",
            initialCc = "carol@test.local",
            initialBcc = "dave@test.local",
            initialSubject = "Lunch on Friday",
        )

        closeComposer()

        compose.onNodeWithText(L10n.compose_discard_title(ctx())).assertDoesNotExist()
        assertEquals(1, closed.size)
    }
}
