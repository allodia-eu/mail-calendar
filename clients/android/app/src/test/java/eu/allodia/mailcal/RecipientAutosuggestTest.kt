// The composer autosuggest text handling, driven directly, no Compose, no cdylib.
//
// The whole difficulty is that To/Cc/Bcc hold a *list* in one string. The two ways to get this
// wrong both destroy user data rather than merely misbehaving: query with the whole field and
// nothing ever matches, or replace the whole field on selection and every recipient already
// entered is silently deleted. Both are covered below.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RecipientAutosuggestTest {

    @Test
    fun `the query is the token after the last comma, not the whole field`() {
        assertEquals("ada", currentRecipientToken("ada"))
        // With a recipient already entered, querying the whole field would match nothing.
        assertEquals("gr", currentRecipientToken("ada@example.test, gr"))
        assertEquals("gr", currentRecipientToken("ada@example.test,gr"))
        // Whitespace around the token is the user's, not the query's.
        assertEquals("gr", currentRecipientToken("ada@example.test,   gr  "))
    }

    @Test
    fun `a field that ends at a separator has no token`() {
        // Otherwise the dropdown would spring open listing everyone the instant a recipient is
        // completed with a comma.
        assertEquals("", currentRecipientToken("ada@example.test, "))
        assertEquals("", currentRecipientToken("ada@example.test,"))
        assertEquals("", currentRecipientToken(""))
        assertEquals("", currentRecipientToken("   "))
    }

    @Test
    fun `accepting a suggestion keeps every recipient already entered`() {
        // The regression that matters: the earlier addresses must survive.
        assertEquals(
            "ada@example.test, grace@example.test, ",
            acceptRecipientSuggestion("ada@example.test, gr", "grace@example.test"),
        )
        assertEquals(
            "ada@example.test, bob@example.test, grace@example.test, ",
            acceptRecipientSuggestion("ada@example.test, bob@example.test, gr", "grace@example.test"),
        )
    }

    @Test
    fun `accepting into an empty field yields just that recipient`() {
        assertEquals("ada@example.test, ", acceptRecipientSuggestion("", "ada@example.test"))
        assertEquals("ada@example.test, ", acceptRecipientSuggestion("ad", "ada@example.test"))
    }

    @Test
    fun `accepting normalises spacing so the next token starts clean`() {
        // A user who typed no space after the comma still gets a well-formed list back.
        assertEquals(
            "ada@example.test, grace@example.test, ",
            acceptRecipientSuggestion("ada@example.test,gr", "grace@example.test"),
        )
        // And a trailing space before the comma is not doubled.
        assertEquals(
            "ada@example.test, grace@example.test, ",
            acceptRecipientSuggestion("ada@example.test , gr", "grace@example.test"),
        )
    }

    @Test
    fun `the dropdown hides once the token is exactly a suggested address`() {
        // Mid-typing: show.
        assertTrue(shouldShowSuggestions("ad", listOf("ada@example.test", "grace@example.test")))
        // Finished: the address is fully typed, so the dropdown would be offering the user what
        // they have already written while covering the field below it.
        assertFalse(shouldShowSuggestions("ada@example.test", listOf("ada@example.test")))
        // Case is not the user's problem.
        assertFalse(shouldShowSuggestions("ADA@example.test", listOf("ada@example.test")))
        // Note the rule is "the token matches a suggestion", not "the token matches the ONLY
        // suggestion", a fully-typed address cannot come back alongside an unrelated one,
        // because the core filters by that same query before ranking.
    }

    @Test
    fun `no token or no suggestions means no dropdown`() {
        assertFalse(shouldShowSuggestions("", listOf("ada@example.test")))
        assertFalse(shouldShowSuggestions("ada@example.test, ", listOf("ada@example.test")))
        assertFalse(shouldShowSuggestions("ad", emptyList()))
    }

    // ---- Pills: the finished recipients, and the round trip back to one field string ----

    @Test
    fun `pills are the finished recipients, never the one being typed`() {
        // The split is the same one autosuggest uses, so what is drawn as a pill and what is being
        // completed cannot disagree about where one recipient ends.
        assertEquals(listOf("ada@example.test"), committedRecipients("ada@example.test, gr"))
        assertEquals(listOf("ada@example.test"), committedRecipients("ada@example.test, "))
        assertEquals(
            listOf("ada@example.test", "bob@example.test"),
            committedRecipients("ada@example.test, bob@example.test, gr"),
        )
        // Nothing finished yet: a half-typed address is text, not a pill.
        assertEquals(emptyList<String>(), committedRecipients("ada"))
        assertEquals(emptyList<String>(), committedRecipients(""))
    }

    @Test
    fun `a stray separator never becomes a blank pill`() {
        assertEquals(listOf("ada@example.test"), committedRecipients("ada@example.test,,"))
        assertEquals(listOf("ada@example.test"), committedRecipients(",ada@example.test,  ,"))
    }

    @Test
    fun `the field string rebuilds from its pills and token`() {
        assertEquals(
            "ada@example.test, bob@example.test, gr",
            recipientFieldText(listOf("ada@example.test", "bob@example.test"), "gr"),
        )
        // A finished list keeps its trailing separator, so the next character typed starts a new
        // pill rather than being appended to the last address.
        assertEquals("ada@example.test, ", recipientFieldText(listOf("ada@example.test"), ""))
        assertEquals("gr", recipientFieldText(emptyList(), "gr"))
        assertEquals("", recipientFieldText(emptyList(), ""))
    }

    @Test
    fun `splitting a field and rebuilding it is lossless`() {
        // The property that keeps the pills honest: whatever the user has typed, drawing it as
        // (pills + token) and reassembling must not silently alter the recipients.
        for (field in listOf(
            "",
            "ada",
            "ada@example.test, ",
            "ada@example.test, gr",
            "ada@example.test, bob@example.test, ",
        )) {
            assertEquals(
                "round trip of \"$field\"",
                field,
                recipientFieldText(committedRecipients(field), currentRecipientToken(field)),
            )
        }
    }

    @Test
    fun `a pre-filled field has nothing in progress`() {
        // The reply-all bug: the field comes from the core, so the trailing-token rule, which
        // guesses what the user is typing, rendered the last recipient as raw text beside one
        // pill, and a Cc holding a single address as no pill at all.
        val to = seededRecipientField("bestuur@example.test, tc@example.test")
        assertEquals(listOf("bestuur@example.test", "tc@example.test"), committedRecipients(to))
        assertEquals("", currentRecipientToken(to))

        val cc = seededRecipientField("rene@example.test")
        assertEquals(listOf("rene@example.test"), committedRecipients(cc))
        assertEquals("", currentRecipientToken(cc))
    }

    @Test
    fun `seeding an empty field leaves it empty, and seeding twice changes nothing`() {
        // Send is gated on a non-blank To, so a lone separator here would enable it over a message
        // addressed to nobody. Idempotence is what lets the composer compare the current field
        // against its opening value to decide whether anything was typed.
        assertEquals("", seededRecipientField(""))
        assertEquals("", seededRecipientField("   "))
        val once = seededRecipientField("ada@example.test, bob@example.test")
        assertEquals(once, seededRecipientField(once))
    }

    @Test
    fun `removing a pill keeps the others and the half-typed token`() {
        val field = "ada@example.test, bob@example.test, gr"
        assertEquals("bob@example.test, gr", removeRecipient(field, 0))
        assertEquals("ada@example.test, gr", removeRecipient(field, 1))
        // Removing the last pill leaves just what is being typed.
        assertEquals("gr", removeRecipient("ada@example.test, gr", 0))
    }

    @Test
    fun `removing an out-of-range pill changes nothing`() {
        // The pill list and a tap on it are a frame apart; a recomposition in between must not
        // crash the composer.
        val field = "ada@example.test, gr"
        assertEquals(field, removeRecipient(field, 5))
        assertEquals(field, removeRecipient(field, -1))
    }

    @Test
    fun `accepting a suggestion leaves nothing in the input, so the caret is at the end`() {
        // Why the caret bug cannot come back for this path: the accepted address becomes a pill and
        // the text input is emptied, so "end of the text" is position zero.
        val after = acceptRecipientSuggestion("ada@example.test, gr", "grace@example.test")
        assertEquals("", currentRecipientToken(after))
        assertEquals(
            listOf("ada@example.test", "grace@example.test"),
            committedRecipients(after),
        )
    }
}
