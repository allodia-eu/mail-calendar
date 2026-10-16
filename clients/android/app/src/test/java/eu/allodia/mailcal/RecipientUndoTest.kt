// Undo in a recipient field, through the composable and real key events (`docs/undo.md` rule 9).
//
// The input's own undo knows only the address being typed: once a comma made it a pill, Ctrl+Z had
// nothing to take back, and a removed pill could not be brought back at all.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
@OptIn(ExperimentalTestApi::class)
class RecipientUndoTest {

    @get:Rule
    val compose = createComposeRule()

    /** The field wired to a parent-owned string that starts as [initial], as the composer holds it. */
    private fun field(initial: String = ""): () -> String {
        var value by mutableStateOf(initial)
        compose.setContent {
            RecipientField(label = "To", value = value, onValue = { value = it }, suggestionsFor = null)
        }
        return { value }
    }

    private val input get() = compose.onNodeWithTag("recipient-input-To")

    /** Presses [key] with each of [held] down, as a keyboard sends a chord. */
    private fun chord(held: List<Key>, key: Key) {
        input.performKeyInput {
            fun press(index: Int) {
                if (index == held.size) pressKey(key) else withKeyDown(held[index]) { press(index + 1) }
            }
            press(0)
        }
        compose.waitForIdle()
    }

    private fun pill(recipient: String) = compose.onNodeWithContentDescription(
        "$recipient, ${L10n.compose_remove_recipient(RuntimeEnvironment.getApplication())}",
    )

    @Test
    fun `undo takes a finished address back to text, then takes the typing back`() {
        val value = field()
        input.performTextInput("ada@example.test")
        compose.waitForIdle()
        input.performTextInput(",")
        compose.waitForIdle()
        pill("ada@example.test").assertExists()

        chord(listOf(Key.CtrlLeft), key = Key.Z)
        assertEquals("ada@example.test", value())
        pill("ada@example.test").assertDoesNotExist()
        chord(listOf(Key.CtrlLeft), key = Key.Z)
        assertEquals("", value())

        chord(listOf(Key.CtrlLeft, Key.ShiftLeft), key = Key.Z)
        chord(listOf(Key.CtrlLeft), key = Key.Y)
        pill("ada@example.test").assertExists()
    }

    @Test
    fun `a removed recipient comes back, and the caret is in the field to ask for it`() {
        val value = field("ada@example.test, grace@example.test, ")
        pill("ada@example.test").performClick()
        compose.waitForIdle()
        assertEquals("grace@example.test, ", value())
        input.assertIsFocused()

        chord(listOf(Key.CtrlLeft), key = Key.Z)
        assertEquals("ada@example.test, grace@example.test, ", value())
    }

    @Test
    fun `the recipients a field opened with are where undo stops`() {
        val value = field("ada@example.test, ")
        input.performClick()
        chord(listOf(Key.CtrlLeft), key = Key.Z)
        assertEquals("ada@example.test, ", value())
    }
}
