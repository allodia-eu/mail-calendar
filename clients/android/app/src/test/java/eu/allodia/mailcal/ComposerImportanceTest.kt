// The composer's importance picker and a message's importance on screen (docs/importance.md):
// the picker lists what the core offers, a pick goes to the core, and a row and an open message
// draw the importance the core decided.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
import uniffi.mailcal_bindings.ComposerImportance
import uniffi.mailcal_bindings.MessageImportance

/** A stand-in for the core's picker: normal to start with, highest first. */
private class FakeImportanceStore : ComposerImportanceStore {
    var selected = MessageImportance.NORMAL

    override fun current() = ComposerImportance(
        selected = selected,
        options = listOf(MessageImportance.HIGH, MessageImportance.NORMAL, MessageImportance.LOW),
        changed = selected != MessageImportance.NORMAL,
    )

    override fun set(importance: MessageImportance): ComposerImportance {
        selected = importance
        return current()
    }
}

@RunWith(RobolectricTestRunner::class)
class ComposerImportanceTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun a_pick_from_the_overflow_goes_to_the_core() {
        val store = FakeImportanceStore()
        compose.setContent {
            AppTheme {
                var picker by remember { mutableStateOf(store.current()) }
                ComposerTopBar(
                    title = "New message",
                    onClose = {},
                    attach = NO_COMPOSER_FILES.attach,
                    onAttach = {},
                    signatureMenu = null,
                    importanceMenu = { expanded, onExpand ->
                        ComposerImportanceMenu(picker, expanded, onExpand) { chosen ->
                            picker = store.set(chosen)
                        }
                    },
                    onSaveDraft = null,
                    onDiscard = null,
                    sendEnabled = true,
                    onSend = {},
                )
            }
        }
        compose.onNodeWithTag(COMPOSER_OVERFLOW_TAG).performClick()
        compose.onNodeWithText(L10n.compose_importance_label(ctx())).performClick()
        compose.waitForIdle()
        compose.onNodeWithText(L10n.compose_importance_low(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.compose_importance_high(ctx())).performClick()
        compose.waitForIdle()

        assertEquals(MessageImportance.HIGH, store.selected)
    }

    @Test
    fun a_banner_says_the_importance_and_normal_draws_none() {
        var shown by mutableStateOf<MessageImportance?>(MessageImportance.HIGH)
        compose.setContent { AppTheme { ImportanceBanner(shown) } }
        compose.onNodeWithText(L10n.reading_importance_high(ctx())).assertIsDisplayed()

        shown = MessageImportance.LOW
        compose.waitForIdle()
        compose.onNodeWithText(L10n.reading_importance_low(ctx())).assertIsDisplayed()

        shown = null
        compose.waitForIdle()
        compose.onNodeWithText(L10n.reading_importance_low(ctx())).assertDoesNotExist()
    }

    @Test
    fun a_row_mark_is_announced_and_normal_draws_none() {
        var importance by mutableStateOf(MessageImportance.HIGH)
        compose.setContent { AppTheme { ImportanceMark(importance) } }
        compose.onNodeWithContentDescription(L10n.a11y_importance_high(ctx())).assertIsDisplayed()

        importance = MessageImportance.NORMAL
        compose.waitForIdle()
        compose.onNodeWithContentDescription(L10n.a11y_importance_high(ctx())).assertDoesNotExist()
    }
}
