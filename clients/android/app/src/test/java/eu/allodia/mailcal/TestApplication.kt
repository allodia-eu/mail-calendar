// The Application every Robolectric test runs in (robolectric.properties). The suite loads no core,
// so a screen that asks the core something while it draws is answered here instead.
package eu.allodia.mailcal

import android.app.Application
import uniffi.mailcal_bindings.EventSwatch

class TestApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        EventSwatches.source = ::testEventSwatch
    }
}

/** Event colours for the suite: a fixed tint, the calendar's colour as the bar, a plain title. */
internal fun testEventSwatch(hex: String, dark: Boolean) = EventSwatch(
    fill = if (dark) "#1f2933" else "#e8eef5",
    accent = hex,
    text = if (dark) "#f5f7fa" else "#1f2933",
)
