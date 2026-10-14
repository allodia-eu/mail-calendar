// An answer given from an event's detail: which of the three is shown as already given, that
// Accept and Maybe go at once, and that Decline asks first (docs/calendar.md, "Answering from the
// calendar").
package eu.allodia.mailcal

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.filterToOne
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ResponseStatus

private const val MAYBE = "Answer maybe to this invitation"
private const val DECLINE = "Decline this invitation"
private const val ACCEPT = "Accept this invitation"

@RunWith(RobolectricTestRunner::class)
class EventAnswerTest {
    @get:Rule val compose = createComposeRule()

    private val answers = mutableListOf<InvitationResponse>()

    private val saving = mutableStateOf(false)

    private fun answer(current: ResponseStatus) {
        compose.setContent {
            EventAnswer(current, "Planning", saving.value) { response, _ -> answers += response }
        }
        compose.waitForIdle()
    }

    @Test
    fun only_the_answer_already_given_is_current() {
        val all = InvitationResponse.values().toList()
        assertEquals(
            listOf(InvitationResponse.ACCEPT),
            all.filter { isCurrentAnswer(ResponseStatus.ACCEPTED, it) },
        )
        assertEquals(
            listOf(InvitationResponse.DECLINE),
            all.filter { isCurrentAnswer(ResponseStatus.DECLINED, it) },
        )
        assertEquals(
            "an unanswered meeting offers all three",
            emptyList<InvitationResponse>(),
            all.filter { isCurrentAnswer(ResponseStatus.NEEDS_ACTION, it) },
        )
    }

    @Test
    fun maybe_answers_at_once() {
        answer(ResponseStatus.ACCEPTED)
        compose.onNodeWithContentDescription(MAYBE).performClick()
        assertEquals(listOf(InvitationResponse.TENTATIVE), answers)
    }

    @Test
    fun decline_asks_first_and_cancel_sends_nothing() {
        answer(ResponseStatus.ACCEPTED)
        compose.onNodeWithContentDescription(DECLINE).performClick()
        compose.onNodeWithText("Decline this meeting?").assertExists()
        assertEquals("Decline answered without asking", emptyList<InvitationResponse>(), answers)

        compose.onNodeWithText("Cancel").performClick()
        compose.onNodeWithText("Decline this meeting?").assertDoesNotExist()
        assertEquals(emptyList<InvitationResponse>(), answers)

        compose.onNodeWithContentDescription(DECLINE).performClick()
        compose.onAllNodesWithText("Decline").filterToOne(hasAnyAncestor(isDialog())).performClick()
        assertEquals(listOf(InvitationResponse.DECLINE), answers)
    }

    @Test
    fun nothing_can_be_pressed_while_an_answer_is_on_its_way() {
        saving.value = true
        answer(ResponseStatus.TENTATIVE)
        compose.onNodeWithContentDescription(ACCEPT).assertIsNotEnabled()
        compose.onNodeWithContentDescription(DECLINE).assertIsNotEnabled()

        saving.value = false
        compose.waitForIdle()
        compose.onNodeWithContentDescription(ACCEPT).assertIsEnabled()
        compose.onNodeWithContentDescription(MAYBE).assertIsNotEnabled()
    }
}
