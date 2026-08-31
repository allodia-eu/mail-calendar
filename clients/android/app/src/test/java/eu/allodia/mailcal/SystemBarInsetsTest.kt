// Screens that render OUTSIDE the mail/calendar Scaffold must clear the system bars themselves.
//
// The app is edge-to-edge and has no say in it (targetSdk 36), so a full-screen composable that
// applies no window insets draws underneath the status bar and, the way this surfaced, underneath
// the navigation bar. On a three-button device that hid the bottom half of "Get started" on the
// welcome screen: the only way forward out of the first screen a new user ever sees. The Scaffold
// branch is fine because Scaffold pads its content; every other branch of the `when` in
// MainActivity is on its own.
//
// The insets are INJECTED here, not assumed. Robolectric reports none by default, so a test that
// just rendered a screen and measured it would pass exactly as happily with the bug in place:
// dispatching a real system-bar inset is the whole reason this can fail.
package eu.allodia.mailcal

import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** A three-button navigation bar, in pixels, the case in the bug report. */
private const val NAV_BAR_PX = 150

/** The status bar above it, so both edges of the screen are covered. */
private const val STATUS_BAR_PX = 90

@RunWith(RobolectricTestRunner::class)
class SystemBarInsetsTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    /** Renders [screen] and hands it a device whose system bars actually occupy space. */
    private fun showBehindSystemBars(screen: @Composable () -> Unit) {
        compose.setContent { AppTheme { screen() } }
        compose.runOnUiThread {
            val bars = WindowInsetsCompat.Builder()
                .setInsets(
                    WindowInsetsCompat.Type.systemBars(),
                    Insets.of(0, STATUS_BAR_PX, 0, NAV_BAR_PX),
                )
                .build()
            ViewCompat.dispatchApplyWindowInsets(compose.activity.window.decorView, bars)
        }
        compose.waitForIdle()
    }

    private fun topOf(text: String) =
        compose.onNodeWithText(text).fetchSemanticsNode().boundsInRoot.top

    private fun bottomOf(text: String) =
        compose.onNodeWithText(text).fetchSemanticsNode().boundsInRoot.bottom

    private fun screenHeight() = compose.onRoot().fetchSemanticsNode().size.height

    @Test
    fun the_welcome_screens_only_way_forward_clears_the_navigation_bar() {
        showBehindSystemBars {
            WelcomeScreen(payloadPreview = { "{}" }, onGetStarted = {})
        }

        val navBarTop = screenHeight() - NAV_BAR_PX
        val button = bottomOf(L10n.welcome_get_started(compose.activity))
        assertTrue(
            "\"Get started\" must not sit under the navigation bar: " +
                "its bottom is $button, the navigation bar starts at $navBarTop",
            button <= navBarTop,
        )
    }

    @Test
    fun the_diagnostics_screen_clears_the_status_bar() {
        showBehindSystemBars { DiagnosticsScreen(onSetLogLevel = {}, onBack = {}) }

        val title = topOf(L10n.settings_category_diagnostics(compose.activity))
        assertTrue(
            "the Diagnostics title must not sit under the clock: its top is $title",
            title >= STATUS_BAR_PX,
        )
    }
}
