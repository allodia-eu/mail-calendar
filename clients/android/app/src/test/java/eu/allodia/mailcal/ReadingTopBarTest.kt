// The reading screen's actions (docs/reading-actions.md): the top app bar holds back, archive,
// delete and the overflow last of all (ReadingTopBar.kt); the answers are large buttons at the foot
// of the screen, the core's default answer first, then Forward, then a chevron holding the other
// answer (ReadingAnswers.kt).
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertTextContains
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

    private fun showBar() {
        rule.setContent {
            AppTheme {
                ReadingTopBar(
                    onBack = { pressed += "back" },
                    onArchive = { pressed += "archive" },
                    onDelete = { pressed += "delete" },
                ) { Text("overflow", modifier = Modifier.testTag(READING_OVERFLOW_TAG)) }
            }
        }
    }

    private fun showAnswers(defaultReplyAll: Boolean) {
        rule.setContent {
            AppTheme {
                ReadingAnswers(
                    defaultReplyAll = defaultReplyAll,
                    onReply = { pressed += "reply" },
                    onReplyAll = { pressed += "replyAll" },
                    onForward = { pressed += "forward" },
                )
            }
        }
    }

    private fun left(node: SemanticsNodeInteraction) = node.fetchSemanticsNode().boundsInRoot.left

    @Test
    fun theBarHoldsArchiveAndDeleteThenTheOverflowLast() {
        showBar()
        val archive = left(rule.onNodeWithContentDescription(L10n.action_archive(ctx)))
        val delete = left(rule.onNodeWithContentDescription(L10n.action_delete(ctx)))
        val overflow = left(rule.onNodeWithTag(READING_OVERFLOW_TAG))
        assertTrue(archive < delete && delete < overflow)
        // The answers are at the foot of the screen, not up here out of a thumb's reach.
        rule.onNodeWithContentDescription(L10n.action_reply(ctx)).assertDoesNotExist()
    }

    @Test
    fun backArchiveAndDeleteAreNamedAndPressable() {
        showBar()
        rule.onNodeWithContentDescription(L10n.a11y_back(ctx)).performClick()
        rule.onNodeWithContentDescription(L10n.action_archive(ctx)).performClick()
        rule.onNodeWithContentDescription(L10n.action_delete(ctx)).performClick()
        assertEquals(listOf("back", "archive", "delete"), pressed)
    }

    @Test
    fun theDefaultAnswerIsReplyUnlessTheCoreSaysEveryone() {
        showAnswers(defaultReplyAll = false)
        rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG).assertTextContains(L10n.action_reply(ctx)).performClick()
        assertEquals(listOf("reply"), pressed)
    }

    @Test
    fun aMessageToSeveralPeopleAnswersEveryoneByDefault() {
        showAnswers(defaultReplyAll = true)
        rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG).assertTextContains(L10n.action_reply_all(ctx)).performClick()
        assertEquals(listOf("replyAll"), pressed)
    }

    @Test
    fun forwardIsABigButtonOfItsOwn() {
        showAnswers(defaultReplyAll = false)
        rule.onNodeWithTag(READING_FORWARD_TAG).assertTextContains(L10n.action_forward(ctx)).performClick()
        assertEquals(listOf("forward"), pressed)
    }

    @Test
    fun theChevronHoldsTheOtherAnswer() {
        showAnswers(defaultReplyAll = true)
        rule.onNodeWithContentDescription(L10n.a11y_more_replies(ctx)).performClick()
        rule.onNodeWithText(L10n.action_reply(ctx)).performClick()
        assertEquals(listOf("reply"), pressed)
    }

    @Test
    fun theAnswersReadDefaultThenForwardThenTheChevron() {
        showAnswers(defaultReplyAll = false)
        val default = left(rule.onNodeWithTag(READING_DEFAULT_ANSWER_TAG))
        val forward = left(rule.onNodeWithTag(READING_FORWARD_TAG))
        val more = left(rule.onNodeWithTag(READING_MORE_REPLIES_TAG))
        assertTrue(default < forward && forward < more)
    }
}
