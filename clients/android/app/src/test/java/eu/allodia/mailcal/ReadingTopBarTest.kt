// The reading screen's actions in the top app bar (ReadingTopBar.kt, docs/reading-actions.md):
// archive and delete, the core's default answer with a chevron holding the others, then the
// overflow last of all.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.material3.Text
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingTopBarTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private val pressed = mutableListOf<String>()

    private fun show(defaultReplyAll: Boolean) {
        rule.setContent {
            AppTheme {
                ReadingTopBar(
                    defaultReplyAll = defaultReplyAll,
                    onBack = { pressed += "back" },
                    onArchive = { pressed += "archive" },
                    onDelete = { pressed += "delete" },
                    onReply = { pressed += "reply" },
                    onReplyAll = { pressed += "replyAll" },
                    onForward = { pressed += "forward" },
                ) { Text("overflow", modifier = Modifier.testTag(READING_OVERFLOW_TAG)) }
            }
        }
    }

    @Test
    fun theDefaultAnswerIsReplyUnlessTheCoreSaysEveryone() {
        show(defaultReplyAll = false)
        rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG)
            .assertContentDescriptionEquals(L10n.action_reply(ctx))
            .performClick()
        assertEquals(listOf("reply"), pressed)
    }

    @Test
    fun aMessageToSeveralPeopleAnswersEveryoneByDefault() {
        show(defaultReplyAll = true)
        rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG)
            .assertContentDescriptionEquals(L10n.action_reply_all(ctx))
            .performClick()
        assertEquals(listOf("replyAll"), pressed)
    }

    @Test
    fun theChevronHoldsTheOtherAnswerAndForward() {
        show(defaultReplyAll = true)
        rule.onNodeWithContentDescription(L10n.a11y_more_replies(ctx)).performClick()
        rule.onNodeWithText(L10n.action_reply(ctx)).performClick()
        rule.onNodeWithContentDescription(L10n.a11y_more_replies(ctx)).performClick()
        rule.onNodeWithText(L10n.action_forward(ctx)).performClick()
        assertEquals(listOf("reply", "forward"), pressed)
        // The default answer is on the bar, never in the menu as well.
        rule.onNodeWithContentDescription(L10n.a11y_more_replies(ctx)).performClick()
        rule.onNodeWithText(L10n.action_reply_all(ctx)).assertDoesNotExist()
    }

    @Test
    fun archiveAndDeleteComeFirstAndTheOverflowLast() {
        show(defaultReplyAll = false)
        fun left(node: androidx.compose.ui.test.SemanticsNodeInteraction) =
            node.fetchSemanticsNode().boundsInRoot.left
        val archive = left(rule.onNodeWithContentDescription(L10n.action_archive(ctx)))
        val delete = left(rule.onNodeWithContentDescription(L10n.action_delete(ctx)))
        val answer = left(rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG))
        val more = left(rule.onNodeWithTag(READING_MORE_REPLIES_TAG))
        val overflow = left(rule.onNodeWithTag(READING_OVERFLOW_TAG))
        assertTrue(archive < delete && delete < answer && answer < more && more < overflow)
    }

    @Test
    fun backArchiveAndDeleteAreNamedAndPressable() {
        show(defaultReplyAll = false)
        rule.onNodeWithContentDescription(L10n.a11y_back(ctx)).performClick()
        rule.onNodeWithContentDescription(L10n.action_archive(ctx)).performClick()
        rule.onNodeWithContentDescription(L10n.action_delete(ctx)).performClick()
        assertEquals(listOf("back", "archive", "delete"), pressed)
    }
}
