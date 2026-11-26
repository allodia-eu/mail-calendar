// Settings → Notifications, drawn as the core decides it (`notifications_page`), and the one-time
// hand-over of a choice an older install kept in SharedPreferences.
//
// What a pass announces while the choice is off is the core's and tested there. What is tested here
// is that the switch draws the page's value and sends a change, and that a stored "off" reaches the
// core exactly once.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.NotificationsPage
import uniffi.mailcal_bindings.PreferenceIntent

/** Settings → Notifications with new-mail notifications on, the core's default. */
internal val NOTIFICATIONS_ON = NotificationsPage(newMail = true)

private const val PREFS = "mailcal_prefs"
private const val KEY = "new_mail_notifications_enabled"

@RunWith(RobolectricTestRunner::class)
class SettingsNotificationsTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()
    private val prefs get() = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val sent = mutableListOf<Any>()

    @Before
    fun clearPrefs() {
        prefs.edit().clear().commit()
    }

    // In a column, as the category's detail screen draws it: otherwise the battery card, drawn
    // when Robolectric reports no exemption, lies over the switch and takes its click.
    private fun show(newMail: Boolean) {
        compose.setContent {
            Column {
                NotificationsSettings(NotificationsPage(newMail = newMail), onSetNewMail = { sent += it })
            }
        }
    }

    @Test
    fun the_switch_draws_on_and_sends_off() {
        show(newMail = true)
        compose.onNodeWithText(L10n.settings_toggle_on(ctx)).assertExists()
        compose.onNodeWithTag(NEW_MAIL_SWITCH).assertIsOn().performClick()
        assertEquals(listOf<Any>(false), sent)
    }

    @Test
    fun the_switch_draws_off_and_sends_on() {
        show(newMail = false)
        compose.onNodeWithText(L10n.settings_toggle_off(ctx)).assertExists()
        compose.onNodeWithTag(NEW_MAIL_SWITCH).assertIsOff().performClick()
        assertEquals(listOf<Any>(true), sent)
    }

    @Test
    fun the_switch_keeps_the_page_value_until_the_core_answers() {
        show(newMail = true)
        compose.onNodeWithTag(NEW_MAIL_SWITCH).performClick()
        compose.onNodeWithTag(NEW_MAIL_SWITCH).assertIsOn()
    }

    @Test
    fun a_stored_off_is_sent_once_and_forgotten() {
        prefs.edit().putBoolean(KEY, false).commit()
        NotificationsChoiceImport.run(ctx) { sent += it }
        assertEquals(listOf<Any>(PreferenceIntent.SetNewMailNotifications(on = false)), sent)
        assertFalse(prefs.contains(KEY))

        NotificationsChoiceImport.run(ctx) { sent += it }
        assertEquals(1, sent.size)
    }

    @Test
    fun a_stored_on_sends_nothing_and_is_forgotten() {
        prefs.edit().putBoolean(KEY, true).commit()
        NotificationsChoiceImport.run(ctx) { sent += it }
        assertEquals(emptyList<Any>(), sent)
        assertFalse(prefs.contains(KEY))
    }

    @Test
    fun nothing_stored_sends_nothing() {
        prefs.edit().putBoolean("diagnostic_log_debug_enabled", true).commit()
        NotificationsChoiceImport.run(ctx) { sent += it }
        assertEquals(emptyList<Any>(), sent)
        assertEquals(true, prefs.getBoolean("diagnostic_log_debug_enabled", false))
    }
}
