// The launch view (docs/boot-sequence.md): while the core opens the mailbox, a blank page until
// the threshold has passed, then the progress indicator and what it is waiting for. A label raised
// and removed inside the threshold reads as flicker, so nothing may show before it.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private const val AFTER_MS = 500L
private const val OPENING = "Opening your mailbox…"

@RunWith(RobolectricTestRunner::class)
class LaunchStatusTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun the_page_stays_blank_until_the_threshold_then_says_what_it_waits_for() {
        compose.mainClock.autoAdvance = false
        compose.setContent { LaunchStatus(AFTER_MS) }

        compose.mainClock.advanceTimeBy(AFTER_MS - 100)
        compose.onNodeWithText(OPENING).assertDoesNotExist()

        compose.mainClock.advanceTimeBy(200)
        compose.onNodeWithText(OPENING).assertIsDisplayed()
    }
}
