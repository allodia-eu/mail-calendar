// Turning the core's resolved calendar colours into Compose colours.
//
// The client picks the theme and nothing else. The core resolves each calendar to a light and a
// dark [Swatch], fill, label, border, and the label is already guaranteed to clear WCAG AA
// against its fill, so no client ever computes contrast and three clients cannot disagree about
// whether a chip is readable. All that is left here is parsing `#rrggbb` and choosing a theme.
package eu.allodia.mailcal

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import java.util.concurrent.ConcurrentHashMap
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.EventSwatch
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.calendarEventSwatch

// The chip a block falls back to when its calendar is missing from the page's calendar list. That
// should not happen, the core lists every calendar it draws from, but a block with no colour at
// all would be invisible, which is a worse failure than a grey one.
private val FALLBACK = Swatch(background = "#6b7280", text = "#ffffff", border = "#4b5563")

/** Parses the core's `#rrggbb`, falling back to a visible grey rather than throwing at draw time. */
internal fun parseHexColor(hex: String): Color {
    val body = hex.removePrefix("#")
    if (body.length != 6) return Color(0xFF6B7280)
    val rgb = body.toLongOrNull(16) ?: return Color(0xFF6B7280)
    return Color(0xFF000000L or rgb)
}

/** The swatch to draw this calendar with in the active theme. */
internal fun CalendarColor.swatch(dark: Boolean): Swatch = if (dark) this.dark else this.light

/**
 * The calendar a block belongs to, for its colour *and* its spoken label ("…, Work").
 *
 * Keyed on account **and** id: a provider key is only unique inside its account, so two accounts can
 * mint the same calendar id, and matching on the id alone would paint one account's event in the
 * other's colour.
 */
internal fun List<CalendarRow>.rowFor(account: String, calendar: String): CalendarRow? =
    firstOrNull { it.account == account && it.id == calendar }

/** The swatch for a looked-up calendar, or the grey fallback when the page didn't list it. */
internal fun CalendarRow?.swatchOrFallback(dark: Boolean): Swatch =
    this?.color?.swatch(dark) ?: FALLBACK

/**
 * How an event wears its calendar's colour (docs/calendar.md §1): the tint that fills it, the bar of
 * the colour down its leading edge, and the title, which the core guarantees clears AA on the tint.
 */
@Immutable
internal data class EventColors(val fill: Color, val accent: Color, val text: Color)

/** The core's event colours, asked once per calendar colour and theme rather than per block. */
internal object EventSwatches {
    /** Where the colours come from: the core. The JVM suite, which loads no core, swaps in its own. */
    @Volatile
    var source: (String, Boolean) -> EventSwatch = ::calendarEventSwatch
        set(value) {
            field = value
            cache.clear()
        }

    private val cache = ConcurrentHashMap<String, EventColors>()

    fun colors(hex: String, dark: Boolean): EventColors = cache.getOrPut("$hex|$dark") {
        val swatch = source(hex, dark)
        EventColors(parseHexColor(swatch.fill), parseHexColor(swatch.accent), parseHexColor(swatch.text))
    }
}

/** The event colours for a looked-up calendar, or the grey fallback's when the page didn't list it. */
internal fun CalendarRow?.eventColorsOrFallback(dark: Boolean): EventColors =
    EventSwatches.colors(this?.color?.hex ?: FALLBACK.background, dark)
