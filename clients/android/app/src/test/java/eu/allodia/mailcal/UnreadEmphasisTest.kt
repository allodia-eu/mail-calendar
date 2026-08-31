// An unread row has to be findable while scanning a full mailbox, so the SUBJECT and the SENDER both
// carry the weight, the accent dot alone is a small target for the eye, and it is the sender that
// tells you whether an unread row is worth opening.
//
// The subject's weight is applied inline at each Text (it is a plain `if` on a Compose parameter);
// the sender's runs through `unreadSenderWeight`, which exists precisely so the decision is written
// once and can be asserted here. Three clients implement this rule separately, and a rule three
// clients implement separately is a rule that drifts.
package eu.allodia.mailcal

import androidx.compose.ui.text.font.FontWeight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class UnreadEmphasisTest {

    @Test
    fun an_unread_sender_is_heavier_than_a_read_one() {
        assertEquals(FontWeight.Bold, unreadSenderWeight(true))
        assertEquals(FontWeight.Medium, unreadSenderWeight(false))
    }

    @Test
    fun the_two_weights_are_actually_distinguishable() {
        // The assertion that would have caught the real mistake: returning the same weight from both
        // arms compiles, renders, and looks exactly like a feature nobody implemented. Compare the
        // numeric weights rather than the objects, so a future swap to (SemiBold, Normal) still
        // proves the ordering rather than just "these two constants differ".
        assertTrue(
            "unread must be strictly heavier than read",
            unreadSenderWeight(true).weight > unreadSenderWeight(false).weight,
        )
    }

    @Test
    fun a_read_sender_is_still_emphasised_above_the_body_preview() {
        // Deliberately Medium and not Normal: the sender shares one Text with the lighter preview
        // snippet, and dropping it to Normal when read would make the READ rows harder to scan than
        // they are today, fixing unread by breaking everything else.
        assertTrue(
            "a read sender must stay above Normal",
            unreadSenderWeight(false).weight > FontWeight.Normal.weight,
        )
    }
}
