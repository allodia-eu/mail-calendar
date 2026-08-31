// The Diagnostics screen's basics: the rows render from the L10n catalog, the debug
// toggle maps ON→DEBUG / OFF→INFO and persists the choice, sharing confirms past the privacy
// note BEFORE anything leaves the device, and the viewer shows the empty state when there is
// nothing to show.
//
// Compose's test rule under Robolectric, like CalendarWriteIndicatorTest. The level mapping is a
// plain function (logLevelForDebug) so the state machine is pinned without composing; the Compose
// tests prove the wiring the mapping cannot, that the switch actually applies it, and that the
// share button opens a confirm step rather than a share sheet.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.LogLevel

@RunWith(RobolectricTestRunner::class)
class DiagnosticsScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    @Test
    fun the_toggle_mapping_is_debug_on_info_off() {
        assertEquals(LogLevel.DEBUG, logLevelForDebug(true))
        assertEquals(LogLevel.INFO, logLevelForDebug(false))
    }

    @Test
    fun the_screen_renders_its_rows_from_the_catalog() {
        compose.setContent { AppTheme { DiagnosticsScreen(onSetLogLevel = {}, onBack = {}) } }

        listOf(
            L10n.diagnostics_log_heading(ctx()),
            L10n.diagnostics_log_size_label(ctx()),
            L10n.diagnostics_log_backups_label(ctx()),
            L10n.diagnostics_log_cap_note(ctx()),
            L10n.diagnostics_view_log(ctx()),
            L10n.diagnostics_share_log(ctx()),
            L10n.diagnostics_copy_path(ctx()),
            L10n.diagnostics_debug_heading(ctx()),
            L10n.diagnostics_debug_description(ctx()),
        ).forEach { compose.onNodeWithText(it).assertExists() }
    }

    @Test
    fun toggling_debug_applies_the_mapped_level_and_persists_the_choice() {
        val applied = mutableListOf<LogLevel>()
        compose.setContent {
            AppTheme { DiagnosticsScreen(onSetLogLevel = { applied += it }, onBack = {}) }
        }

        compose.onNode(isToggleable()).performScrollTo().performClick()
        assertTrue(DiagnosticsPrefs.debugEnabled(ctx()))

        compose.onNode(isToggleable()).performClick()
        assertFalse(DiagnosticsPrefs.debugEnabled(ctx()))

        assertEquals(listOf(LogLevel.DEBUG, LogLevel.INFO), applied)
    }

    @Test
    fun sharing_surfaces_the_privacy_note_before_anything_leaves_the_device() {
        compose.setContent { AppTheme { DiagnosticsScreen(onSetLogLevel = {}, onBack = {}) } }

        compose.onNodeWithText(L10n.diagnostics_share_log(ctx())).performScrollTo().performClick()

        compose.onNodeWithText(L10n.diagnostics_share_confirm_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.diagnostics_share_privacy_note(ctx())).assertIsDisplayed()

        // Cancel backs out: the note disappears and no share sheet was opened.
        compose.onNodeWithText(L10n.action_cancel(ctx())).performClick()
        compose.onNodeWithText(L10n.diagnostics_share_privacy_note(ctx())).assertDoesNotExist()
    }

    @Test
    fun the_viewer_shows_the_empty_state_when_there_is_nothing_yet() {
        compose.setContent { AppTheme { DiagnosticsScreen(onSetLogLevel = {}, onBack = {}) } }

        compose.onNodeWithText(L10n.diagnostics_view_log(ctx())).performScrollTo().performClick()

        compose.onNodeWithText(L10n.diagnostics_log_empty(ctx())).assertIsDisplayed()
    }
}
