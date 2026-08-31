// The calendar write-status badge: the mapping from the core's CalendarWriteStatus to what the
// header shows, and that a failed write renders a tap-to-retry warning.
//
// The mapping is a plain, Compose-free function so the state machine is pinned without composing a
// screen (a synthetic tap on a warning icon cannot tell you the mapping is right). The one Compose
// test proves the wiring the mapping cannot: that the warning is actually tappable and its tap runs
// the retry, a RefreshCalendar, not a re-send.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.CalendarWriteStatus

@RunWith(RobolectricTestRunner::class)
class CalendarWriteIndicatorTest {
    @get:Rule
    val compose = createComposeRule()

    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    @Test
    fun every_status_maps_to_an_indicator() {
        assertEquals(
            CalendarWriteIndicator.Hidden,
            CalendarWriteIndicator.of(CalendarWriteStatus.IDLE),
        )
        assertEquals(
            CalendarWriteIndicator.Spinner,
            CalendarWriteIndicator.of(CalendarWriteStatus.SAVING),
        )
        assertEquals(
            CalendarWriteIndicator.Saved,
            CalendarWriteIndicator.of(CalendarWriteStatus.SAVED),
        )
        assertEquals(
            CalendarWriteIndicator.Warning,
            CalendarWriteIndicator.of(CalendarWriteStatus.FAILED),
        )
    }

    @Test
    fun only_the_warning_offers_a_retry() {
        // The retry is a refresh, and it only makes sense on the unconfirmed state. Offering it on a
        // spinner or a check would invite the user to "retry" a write that is fine.
        assertTrue(CalendarWriteIndicator.Warning.offersRetry)
        assertFalse(CalendarWriteIndicator.Spinner.offersRetry)
        assertFalse(CalendarWriteIndicator.Saved.offersRetry)
        assertFalse(CalendarWriteIndicator.Hidden.offersRetry)
    }

    @Test
    fun a_failed_write_shows_a_warning_whose_tap_retries() {
        val label = L10n.calendar_save_unconfirmed(ctx())
        var retries = 0
        compose.setContent {
            AppTheme {
                CalendarWriteBadge(
                    status = CalendarWriteStatus.FAILED,
                    onRetry = { retries += 1 },
                )
            }
        }

        compose.onNodeWithContentDescription(label).assertIsDisplayed().performClick()
        assertEquals(1, retries)
    }

    @Test
    fun a_saving_write_shows_a_spinner() {
        compose.setContent {
            AppTheme {
                CalendarWriteBadge(status = CalendarWriteStatus.SAVING, onRetry = {})
            }
        }
        compose.onNodeWithContentDescription(L10n.calendar_saving(ctx())).assertIsDisplayed()
    }

    @Test
    fun an_idle_write_shows_nothing() {
        compose.setContent {
            AppTheme {
                CalendarWriteBadge(status = CalendarWriteStatus.IDLE, onRetry = {})
            }
        }
        // None of the three visible states are present: the header stays clean.
        compose.onNodeWithContentDescription(L10n.calendar_saving(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.calendar_saved(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.calendar_save_unconfirmed(ctx()))
            .assertDoesNotExist()
    }
}
