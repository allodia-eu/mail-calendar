// Where the composer's caret opens, and what the suggestion list is allowed to move.
//
// Both are cross-platform rules (docs/contacts.md §4) and both failed silently here: the header
// looked correct in a screenshot while the keyboard had gone nowhere, and the list looked correct
// while everything under it jumped down a whole field's height on each keystroke.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.RecipientMatch

private val ALICE = AccountRow("acct-1", "alice@test.local", expanded = true)

private val PHONE_WIDTH = 360.dp

// One matching address is all the layout assertion needs: even a single row is a whole field's
// worth of height, and that is what used to displace the form.
private val MATCHES = listOf(
    RecipientMatch(email = "bob@test.local", displayName = "Bob Tester", isSaved = true),
)

@RunWith(RobolectricTestRunner::class)
class ComposerRecipientFocusTest {
    @get:Rule val compose = createComposeRule()

    private fun header(to: String, focusesTo: Boolean, width: Dp = PHONE_WIDTH) {
        var value by mutableStateOf(to)
        compose.setContent {
            Box(modifier = Modifier.width(width)) {
                ComposerHeaderFields(
                    accounts = listOf(ALICE),
                    from = ALICE,
                    onFrom = {},
                    to = value,
                    onTo = { value = it },
                    cc = "",
                    onCc = {},
                    bcc = "",
                    onBcc = {},
                    subject = "",
                    onSubject = {},
                    showCcBcc = false,
                    onToggleCcBcc = {},
                    style = null,
                    onStyle = {},
                    suggestionsFor = { query -> MATCHES.filter { it.email.startsWith(query) } },
                    focusesTo = focusesTo,
                )
            }
        }
        compose.waitForIdle()
    }

    private fun suggestionsAreUp(): Boolean =
        compose.onAllNodesWithTag("recipient-suggestions").fetchSemanticsNodes().isNotEmpty()

    @Test
    fun a_new_message_opens_with_the_caret_in_to() {
        header(to = "", focusesTo = true)

        compose.onNodeWithTag("recipient-input-To").assertIsFocused()
    }

    @Test
    fun an_addressed_composer_leaves_to_alone() {
        // A reply, a reply-all's derived recipients, an assistant's draft: the body takes the caret
        // instead, so To must not steal it back.
        header(to = "bob@test.local, ", focusesTo = false)

        compose.onNodeWithTag("recipient-input-To").assertIsNotFocused()
    }

    @Test
    fun the_caret_opens_in_the_body_only_for_a_composer_that_is_already_addressed() {
        // The predicate both halves read, so a client cannot focus two places or neither.
        assertFalse(composerOpensInBody(RichComposeMode.New, ""))
        assertFalse("whitespace is not an address", composerOpensInBody(RichComposeMode.New, "  "))
        // A mail link, or an assistant's draft: a new message that arrived addressed.
        assertTrue(composerOpensInBody(RichComposeMode.New, "bob@test.local"))
        assertTrue(composerOpensInBody(RichComposeMode.Reply, "bob@test.local"))
        assertTrue(composerOpensInBody(RichComposeMode.ReplyAll, "bob@test.local"))
        // A forward carries the quoted original and no recipient. The body still takes the caret:
        // the note above the quote is what the user came to write, and the Send button already
        // says the message is unaddressed.
        assertTrue(composerOpensInBody(RichComposeMode.Forward, ""))
    }

    @Test
    fun only_the_focused_field_offers_suggestions() {
        header(to = "", focusesTo = true)
        compose.onNodeWithTag("recipient-input-To").performTextInput("bob")
        compose.waitUntil(timeoutMillis = 5_000) { suggestionsAreUp() }

        // Moving on must take To's list with it. Harmless while the list sat in the layout; now
        // that it floats it would hang over whatever the user moved to (docs/contacts.md §4).
        compose.onNodeWithText("Subject").performClick()
        compose.waitUntil(timeoutMillis = 5_000) { !suggestionsAreUp() }
    }

    @Test
    fun the_suggestion_list_does_not_displace_the_form() {
        header(to = "", focusesTo = true)
        val before = compose.onNodeWithText("Subject").getUnclippedBoundsInRoot()

        compose.onNodeWithTag("recipient-input-To").performTextInput("bob")
        compose.waitUntil(timeoutMillis = 5_000) { suggestionsAreUp() }

        compose.onNodeWithTag("recipient-suggestions").assertIsDisplayed()
        val after = compose.onNodeWithText("Subject").getUnclippedBoundsInRoot()
        // The whole contract: the list floats, so nothing under it moved. Inline, Subject dropped
        // by the list's full height, and on the phone the header's measured height is the
        // editor's top inset, so the message body went down with it.
        assertEquals(before.top.value, after.top.value, 0.5f)
    }
}
