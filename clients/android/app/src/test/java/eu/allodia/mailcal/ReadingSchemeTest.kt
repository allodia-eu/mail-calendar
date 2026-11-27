// Which page a message is drawn on, and the two controls that change it: the first item of the
// reading screen's overflow and the switch in Settings → Reading (ReadingScheme.kt,
// ReadingScreenOverflow.kt, docs/reading-dark-mode.md).
//
// The reading screen itself reaches the cdylib for its canvas, which this suite does not load, so
// the rule and the controls are asserted here, where they are plain functions and composables.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.MessageScheme

@RunWith(RobolectricTestRunner::class)
class ReadingSchemeTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun aMessageIsDarkOnlyInADarkAppWithDarkMessagesLeftOn() {
        assertEquals(MessageScheme.DARK, messageScheme(appDark = true, darkMessages = true))
        assertEquals(MessageScheme.LIGHT, messageScheme(appDark = true, darkMessages = false))
        // A light app has nothing to darken, whatever the preference says.
        assertEquals(MessageScheme.LIGHT, messageScheme(appDark = false, darkMessages = true))
        assertEquals(MessageScheme.LIGHT, messageScheme(appDark = false, darkMessages = false))
    }

    private fun overflow(scheme: MessageScheme?, onSet: (Boolean) -> Unit = {}) {
        rule.setContent {
            ReadingOverflowMenu(
                subject = "Quarterly report",
                account = "acct-1",
                key = "m1",
                onExportMessage = { _, _, _ -> true },
                onPrint = {},
                scheme = scheme,
                onSetDarkMessages = onSet,
            )
        }
        rule.onNodeWithTag(READING_OVERFLOW_TAG).performClick()
    }

    @Test
    fun theSunOnADarkMessageTurnsDarkMessagesOff() {
        var set: Boolean? = null
        overflow(MessageScheme.DARK) { set = it }
        rule.onNodeWithText(L10n.reading_show_light_background(ctx)).performClick()
        assertEquals(false, set)
    }

    @Test
    fun theMoonOnALightMessageTurnsDarkMessagesOn() {
        var set: Boolean? = null
        overflow(MessageScheme.LIGHT) { set = it }
        rule.onNodeWithText(L10n.reading_show_dark_background(ctx)).performClick()
        assertEquals(true, set)
    }

    @Test
    fun theChoiceOpensTheOverflowAboveWhatActsOnTheMessage() {
        overflow(MessageScheme.DARK)
        val choice = rule.onNodeWithText(L10n.reading_show_light_background(ctx)).fetchSemanticsNode()
        val export = rule.onNodeWithText(L10n.action_save_as_eml(ctx)).fetchSemanticsNode()
        assertTrue(choice.boundsInRoot.top < export.boundsInRoot.top)
    }

    @Test
    fun aLightAppOffersNoChoice() {
        // A light app has nothing to darken (rule 3), so the menu holds only the document actions.
        overflow(scheme = null)
        rule.onNodeWithText(L10n.reading_show_light_background(ctx)).assertDoesNotExist()
        rule.onNodeWithText(L10n.reading_show_dark_background(ctx)).assertDoesNotExist()
        rule.onNodeWithText(L10n.action_save_as_eml(ctx)).assertExists()
    }

    @Test
    fun theSettingsSwitchShowsAndSetsTheSamePreference() {
        var set: Boolean? = null
        rule.setContent { DarkMessagesSwitch(checked = true) { set = it } }
        val row = rule.onNodeWithText(L10n.settings_dark_messages_toggle(ctx))
        row.performClick()
        assertEquals(false, set)
    }
}
