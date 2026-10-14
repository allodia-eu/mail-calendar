// Which page a message is drawn on, and the two controls that change it
// (ReadingScheme.kt, docs/reading-dark-mode.md).
//
// The reading screen itself reaches the cdylib for its canvas, which this suite does not load, so
// the rule and the controls are asserted here, where they are plain functions and composables.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
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

    @Test
    fun theSunOnADarkMessageTurnsDarkMessagesOff() {
        var set: Boolean? = null
        rule.setContent { MessageSchemeToggle(MessageScheme.DARK) { set = it } }
        rule.onNodeWithContentDescription(L10n.reading_show_light_background(ctx)).performClick()
        assertEquals(false, set)
    }

    @Test
    fun theMoonOnALightMessageTurnsDarkMessagesOn() {
        var set: Boolean? = null
        rule.setContent { MessageSchemeToggle(MessageScheme.LIGHT) { set = it } }
        rule.onNodeWithContentDescription(L10n.reading_show_dark_background(ctx)).performClick()
        assertEquals(true, set)
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
