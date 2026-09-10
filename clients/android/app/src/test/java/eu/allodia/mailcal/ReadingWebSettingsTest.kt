// The reading host's WebSettings, which are contract cells rather than preferences
// (docs/rendering-security.md, docs/reading-zoom.md).
//
// Every flag here is one the platform already has an opinion about, and the two that matter most
// are the ones whose default is the *wrong* answer: `useWideViewPort` is false, which makes the
// engine ignore the shared document's viewport tag outright, and `builtInZoomControls` is false,
// which leaves the reader no pinch. Neither failure is visible in a screenshot of a message that
// happens to be narrow, and the WebView cannot be laid out under Robolectric at all (there is no
// renderer), so what this suite can hold is the settings themselves.
//
// Robolectric, because WebSettings is an Android type; the policy itself is plain Kotlin.
package eu.allodia.mailcal

import android.webkit.WebView
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingWebSettingsTest {

    private fun readingSettings() =
        WebView(ApplicationProvider.getApplicationContext()).settings.apply { applyReadingPolicy() }

    @Test
    fun the_native_rendering_gates_are_shut() {
        val settings = readingSettings()

        assertFalse("mail is hostile input: scripting never runs", settings.javaScriptEnabled)
        assertFalse(settings.allowFileAccess)
        assertFalse(settings.allowContentAccess)
    }

    @Test
    fun the_message_lays_out_against_the_pane_and_is_scaled_to_fit_it() {
        val settings = readingSettings()

        // Without this the viewport tag is ignored and a newsletter's own `@media (max-width: …)`
        // rules never fire, so it lays out at its authored width on a phone and runs off the edge.
        assertTrue(
            "the shared document's width=device-width has to reach the engine",
            settings.useWideViewPort,
        )
        // And this is the half no `@media` rule can reach: mail pinned to a fixed width is scaled
        // down until it fits rather than clipped. It does nothing without the wide viewport above,
        // so the pair is asserted together.
        assertTrue(settings.loadWithOverviewMode)
    }

    @Test
    fun the_reader_keeps_a_pinch_and_gets_no_floating_buttons_over_the_message() {
        val settings = readingSettings()

        assertTrue(settings.builtInZoomControls)
        // The legacy on-screen +/- pair is a WebView overlay drawn on top of the message; the
        // gesture is the whole feature and the buttons are not.
        assertFalse(settings.displayZoomControls)
    }
}
