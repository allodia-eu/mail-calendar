// The recipient field's round trip: what the user types, what the parent stores, and what comes
// back. `RecipientAutosuggestTest` covers the pure string functions; this covers the two things they
// cannot, the composable's re-seed guard, which sits between them and the keyboard, and what the
// composer hands the field when it opens.
//
// The guard exists because accepting a suggestion or removing a pill rewrites the text underneath
// the user, and the caret has to follow. The trap is that the value it compares against has been
// through `currentRecipientToken`, which TRIMS, so a guard that compares raw text throws away
// every space the user types, and does it silently.
package eu.allodia.mailcal

import android.text.InputType
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.ComposeKind

@RunWith(RobolectricTestRunner::class)
class RecipientFieldTest {

    @get:Rule
    val compose = createComposeRule()

    /** The view the field is drawn in, which is what the keyboard asks about the focused field. */
    private var view: View? = null

    /** The field wired to a parent-owned string, exactly as the composer holds it. */
    private fun field(onEach: (String) -> Unit = {}): () -> String {
        var value by mutableStateOf("")
        compose.setContent {
            view = LocalView.current
            RecipientField(
                label = "To",
                value = value,
                onValue = {
                    value = it
                    onEach(it)
                },
                suggestionsFor = null,
            )
        }
        return { value }
    }

    @Test
    fun `a typed name keeps its spaces`() {
        // The regression. Typing "Ada " round-trips through the parent as the token "Ada", so a
        // guard comparing untrimmed text fired and rewrote the field without the space. Every
        // space vanished as it was typed: "Ada Lovelace" arrived as "AdaLovelace", and a
        // name-based autosuggest query could never match anything.
        val value = field()

        // Two inserts, because the bug needs the trailing space to exist for one recomposition:
        // a single paste of the whole name never trims and so never reproduced it.
        compose.onNodeWithTag("recipient-input-To").performTextInput("Ada ")
        compose.waitForIdle()
        compose.onNodeWithTag("recipient-input-To").performTextInput("Lovelace")
        compose.waitForIdle()

        assertEquals("Ada Lovelace", value())
    }

    @Test
    fun `a comma still finishes the recipient and empties the input`() {
        // The other half: the guard must still fire when the token GENUINELY changes, or the
        // completed address would stay in the input as well as becoming a pill.
        val value = field()

        compose.onNodeWithTag("recipient-input-To").performTextInput("ada@example.test,")
        compose.waitForIdle()
        compose.onNodeWithText("ada@example.test").assertIsDisplayed()

        // If the input had not been emptied, this would land inside the finished address.
        compose.onNodeWithTag("recipient-input-To").performTextInput("gr")
        compose.waitForIdle()
        assertEquals("ada@example.test, gr", value())
    }

    @Test
    fun `the field asks for the email keyboard`() {
        // What the keyboard is told when the field takes focus: the email variation, which puts
        // `@` on the first page, with no capitalising and no correcting of an address.
        field()
        compose.onNodeWithTag("recipient-input-To").performClick()
        compose.waitForIdle()

        val info = EditorInfo()
        checkNotNull(checkNotNull(view).onCreateInputConnection(info)) { "no input session" }
        val type = info.inputType
        assertEquals(InputType.TYPE_CLASS_TEXT, type and InputType.TYPE_MASK_CLASS)
        assertEquals(
            InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
            type and InputType.TYPE_MASK_VARIATION,
        )
        assertEquals(0, type and InputType.TYPE_TEXT_FLAG_AUTO_CORRECT)
        assertEquals(0, type and InputType.TYPE_TEXT_FLAG_CAP_SENTENCES)
    }

    /** The pill's remove control names its recipient, the only node that exists per PILL. */
    private fun pill(recipient: String) = compose.onNodeWithContentDescription(
        "$recipient, ${L10n.compose_remove_recipient(RuntimeEnvironment.getApplication())}",
    )

    @Test
    fun `every recipient a reply-all opens with is a pill`() {
        // The regression, at the seam where it happened: the composer seeded the field with the
        // core's raw "a, b" and the field's trailing-token rule read the last address as one the
        // user was halfway through typing. So a reply-all showed ONE pill and a loose address, and
        // a Cc with a single recipient showed no pill at all, the fields looked like they had
        // dropped the people they were in fact holding. Asserting on the pills' own remove
        // controls, because the loose text renders a Text node too and "the address is on screen"
        // was true throughout the bug.
        compose.setContent {
            AppTheme {
                RichComposeMessageDialog(
                    request = ComposeContext(
                        testOpening(
                            kind = ComposeKind.REPLY_ALL,
                            to = "bestuur@example.test, tc@example.test",
                            cc = "rene@example.test",
                        ),
                        initialFrom = null,
                    ),
                    onDismiss = {},
                    onSubmitRich = { _ -> true },
                    accounts = emptyList(),
                )
            }
        }
        compose.waitForIdle()

        pill("bestuur@example.test").assertExists()
        pill("tc@example.test").assertExists()
        pill("rene@example.test").assertExists()
    }

    @Test
    fun `a name typed into a field that already has a recipient keeps its spaces too`() {
        // The same trap one recipient later, where the token is no longer the whole field.
        val value = field()

        compose.onNodeWithTag("recipient-input-To").performTextInput("ada@example.test,")
        compose.waitForIdle()
        compose.onNodeWithTag("recipient-input-To").performTextInput("Grace ")
        compose.waitForIdle()
        compose.onNodeWithTag("recipient-input-To").performTextInput("Hopper")
        compose.waitForIdle()

        assertEquals("ada@example.test, Grace Hopper", value())
    }
}
