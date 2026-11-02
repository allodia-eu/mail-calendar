package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.ThreadMessage

// A message in the conversation strip names its sender, and only its sender: one with no `From`,
// as a draft another client stored can be, draws no sender rather than a subject's placeholder. A
// draft says so, and is never called sent.
@RunWith(RobolectricTestRunner::class)
class ConversationStripTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun a_message_without_a_sender_is_not_labelled_with_the_no_subject_placeholder() {
        val unsent = message("unsent", from = "")
        val focused = message("focused", from = "Ada Lovelace")
        compose.setContent {
            AppTheme {
                ConversationStrip(
                    conversation = listOf(focused, unsent),
                    focusedKey = focused.key,
                    subject = "A conversation",
                    activeZoneId = "Europe/Amsterdam",
                    onOpen = { _, _ -> },
                )
            }
        }

        val placeholder = L10n.mail_no_subject(RuntimeEnvironment.getApplication())
        compose.onNodeWithText("Numbers and notes", useUnmergedTree = true).assertExists()
        compose.onNodeWithText(placeholder, useUnmergedTree = true).assertDoesNotExist()
    }

    @Test
    fun a_draft_is_badged_as_a_draft_and_not_as_sent() {
        val draft = message("draft", from = "Ada Lovelace", draft = true)
        val focused = message("focused", from = "Ada Lovelace")
        compose.setContent {
            AppTheme {
                ConversationStrip(
                    conversation = listOf(focused, draft),
                    focusedKey = focused.key,
                    subject = "A conversation",
                    activeZoneId = "Europe/Amsterdam",
                    onOpen = { _, _ -> },
                )
            }
        }

        val ctx = RuntimeEnvironment.getApplication()
        compose.onNodeWithText(L10n.thread_draft(ctx), useUnmergedTree = true).assertExists()
        compose.onNodeWithText(L10n.thread_sent(ctx), useUnmergedTree = true).assertDoesNotExist()
    }

    // Each message on the conversation screen draws its own flag (docs/row-actions.md, rule 3),
    // and a draft among them resumes in a composer wherever the conversation was opened from
    // (docs/drafts.md).
    @Test
    fun a_flagged_message_draws_its_flag_and_a_draft_opens_as_one() {
        val draft = message("draft", from = "Ada Lovelace", draft = true)
        val flagged = message("flagged", from = "Grace Hopper", flagged = true)
        val focused = message("focused", from = "Ada Lovelace")
        val opened = mutableListOf<Pair<String, Boolean>>()
        compose.setContent {
            AppTheme {
                ConversationStrip(
                    conversation = listOf(focused, flagged, draft),
                    focusedKey = focused.key,
                    subject = "A conversation",
                    activeZoneId = "Europe/Amsterdam",
                    onOpen = { message, isDraft -> opened += message.key to isDraft },
                )
            }
        }

        val ctx = RuntimeEnvironment.getApplication()
        compose.onAllNodesWithContentDescription(L10n.a11y_flagged(ctx), useUnmergedTree = true)
            .assertCountEquals(1)
        compose.onNodeWithText("Grace Hopper", useUnmergedTree = true).performClick()
        compose.onNodeWithText(L10n.thread_draft(ctx), useUnmergedTree = true).performClick()
        assertEquals(listOf("flagged" to false, "draft" to true), opened)
    }

    private fun message(
        key: String,
        from: String,
        draft: Boolean = false,
        flagged: Boolean = false,
    ) = ThreadMessage(
        account = "acct-1",
        key = key,
        from = from,
        avatar = stubAvatar("AL"),
        date = "2026-07-10T11:34:41Z",
        preview = "Numbers and notes",
        unread = false,
        flagged = flagged,
        outgoing = false,
        draft = draft,
        hasAttachment = false,
    )
}
