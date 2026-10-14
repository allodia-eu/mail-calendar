// Which of the three answers an event's detail shows as the one already given.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ResponseStatus

class EventAnswerTest {
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
}
