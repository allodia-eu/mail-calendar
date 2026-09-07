// The client-side half of the sender-name step (docs/sending.md): what the field shows while the
// provider is being asked, and what happens when its answer arrives late.
//
// The name itself, its sanitising and its journey to the `From` header are the core's and are
// tested there. What only this layer can get wrong is overwriting somebody mid-sentence.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SenderNameStepTest {

    @Test
    fun `the field starts empty and disabled until the provider answers`() {
        // A field that accepted typing before the suggestion landed would take a keystroke and
        // then throw it away.
        val state = SenderNameStepState("acct-1")

        assertEquals("", state.name)
        assertTrue(state.loading)
    }

    @Test
    fun `a suggestion fills the empty field in`() {
        val state = SenderNameStepState("acct-1")

        state.suggested("Ada Lovelace")

        assertEquals("Ada Lovelace", state.name)
        assertFalse(state.loading)
    }

    @Test
    fun `a late suggestion never overwrites what the user has typed`() {
        // The provider round trip can outlast the user's patience. Replacing their words with
        // the server's, seconds after they started, is worse than never having asked.
        val state = SenderNameStepState("acct-1")
        state.name = "Renée"

        state.suggested("Ada Lovelace")

        assertEquals("Renée", state.name)
        assertFalse(state.loading)
    }

    @Test
    fun `an account with no server-side name still enables the field`() {
        // The ordinary IMAP case: the answer is empty, and it still means "your turn".
        val state = SenderNameStepState("acct-1")

        state.suggested("")

        assertEquals("", state.name)
        assertFalse(state.loading)
    }

    @Test
    fun `the step knows which account it is asking about`() {
        assertEquals("acct-2", SenderNameStepState("acct-2").account)
    }
}
