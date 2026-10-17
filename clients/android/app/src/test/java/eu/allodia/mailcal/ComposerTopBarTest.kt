// The composer's app bar: Attach and Send are drawn in it, and the exceptions (saving a draft by
// hand, changing the signature, throwing the message away) wait in the overflow menu.
package eu.allodia.mailcal

import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class ComposerTopBarTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val pressed = mutableListOf<String>()

    private fun bar(keepsDrafts: Boolean = true, signatures: Boolean = true) {
        compose.setContent {
            AppTheme {
                ComposerTopBar(
                    title = "New message",
                    onClose = { pressed += "close" },
                    onAttach = { pressed += "attach" },
                    signatureMenu = if (signatures) {
                        { expanded, onExpand ->
                            DropdownMenu(expanded = expanded, onDismissRequest = { onExpand(false) }) {
                                DropdownMenuItem(text = { Text("Work") }, onClick = { onExpand(false) })
                            }
                        }
                    } else {
                        null
                    },
                    onSaveDraft = { pressed += "save" }.takeIf { keepsDrafts },
                    onDiscard = { pressed += "discard" }.takeIf { keepsDrafts },
                    sendEnabled = true,
                    onSend = { pressed += "send" },
                )
            }
        }
        compose.waitForIdle()
    }

    private fun openOverflow() {
        compose.onNodeWithTag(COMPOSER_OVERFLOW_TAG).performClick()
        compose.waitForIdle()
    }

    @Test
    fun attach_and_send_are_in_the_bar_and_the_exceptions_are_not() {
        bar()

        compose.onNodeWithContentDescription(L10n.action_attach(ctx())).assertIsDisplayed()
        compose.onNodeWithContentDescription(L10n.action_send(ctx())).assertIsDisplayed()
        compose.onNodeWithContentDescription(L10n.action_save_draft(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.action_discard(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.compose_signature_label(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.action_save_draft(ctx())).assertDoesNotExist()
    }

    @Test
    fun the_overflow_offers_save_signature_and_discard() {
        bar()
        openOverflow()

        compose.onNodeWithText(L10n.action_save_draft(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.compose_signature_label(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.action_discard(ctx())).performClick()
        compose.waitForIdle()

        assertEquals(listOf("discard"), pressed)
    }

    @Test
    fun signature_opens_the_signature_menu() {
        bar()
        openOverflow()

        compose.onNodeWithText(L10n.compose_signature_label(ctx())).performClick()
        compose.waitForIdle()

        compose.onNodeWithText("Work").assertIsDisplayed()
    }

    /** A composer that keeps no drafts and has no signatures has nothing to put behind More. */
    @Test
    fun there_is_no_overflow_with_nothing_in_it() {
        bar(keepsDrafts = false, signatures = false)

        compose.onNodeWithTag(COMPOSER_OVERFLOW_TAG).assertDoesNotExist()
    }
}
