// Return and leaving the field finish an address as a comma does (`docs/contacts.md`): a phone's
// email keyboard has no comma on its first page.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.TextField
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.requestFocus
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class RecipientFinishTest {

    @get:Rule
    val compose = createComposeRule()

    /** The field beside a second one to leave it for, as the composer holds To beside Subject. */
    private fun field(onSubmit: (() -> Unit)? = null): () -> String {
        var value by mutableStateOf("")
        compose.setContent {
            Column {
                RecipientField(
                    label = "To",
                    value = value,
                    onValue = { value = it },
                    suggestionsFor = null,
                    onSubmit = onSubmit,
                )
                TextField(value = "", onValueChange = {}, modifier = Modifier.testTag("subject"))
            }
        }
        return { value }
    }

    private val input get() = compose.onNodeWithTag("recipient-input-To")

    private fun pill(recipient: String) = compose.onNodeWithContentDescription(
        "$recipient, ${L10n.compose_remove_recipient(RuntimeEnvironment.getApplication())}",
    )

    @Test
    fun `return finishes the address and keeps the field`() {
        val value = field()
        input.performTextInput("ada@example.test")
        input.performImeAction()
        compose.waitForIdle()
        assertEquals("ada@example.test, ", value())
        pill("ada@example.test").assertExists()
        input.assertIsFocused()
    }

    @Test
    fun `return in an empty field finishes nothing`() {
        val value = field()
        input.requestFocus()
        input.performImeAction()
        compose.waitForIdle()
        assertEquals("", value())
    }

    @Test
    fun `leaving the field finishes the address`() {
        val value = field()
        input.performTextInput("ada@example.test")
        compose.onNodeWithTag("subject").requestFocus()
        compose.waitForIdle()
        assertEquals("ada@example.test, ", value())
        pill("ada@example.test").assertExists()
    }

    @Test
    fun `a host that finishes the address itself is asked on return, and not on leaving`() {
        var submitted = 0
        val value = field(onSubmit = { submitted++ })
        input.performTextInput("ada@example.test")
        input.performImeAction()
        compose.onNodeWithTag("subject").requestFocus()
        compose.waitForIdle()
        assertEquals(1, submitted)
        assertEquals("ada@example.test", value())
    }
}
