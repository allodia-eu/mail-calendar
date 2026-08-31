// Turning the core's resolved calendar colours into Compose colours.
//
// The core already did the hard part, it picked the palette entry and proved the label clears WCAG
// AA against its fill. The only thing that can go wrong on this side is looking the *wrong calendar*
// up, which paints an event in another calendar's colour and is invisible until someone notices
// their work meetings are green.
package eu.allodia.mailcal

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.Swatch

private fun row(account: String, id: String, name: String, hex: String) = CalendarRow(
    account = account,
    id = id,
    name = name,
    color = CalendarColor(
        hex = hex,
        light = Swatch(hex, "#ffffff", hex),
        dark = Swatch("#111111", "#ffffff", hex),
    ),
    visible = true,
    canWrite = true,
    isDefault = false,
)

class CalendarColorsTest {

    @Test
    fun the_cores_hex_becomes_an_opaque_compose_color() {
        assertEquals(Color(0xFF2F6FA8), parseHexColor("#2f6fa8"))
        assertEquals(Color(0xFF2F6FA8), parseHexColor("2f6fa8"))
        assertEquals(Color(0xFFFFFFFF), parseHexColor("#ffffff"))
        assertEquals(Color(0xFF000000), parseHexColor("#000000"))
    }

    @Test
    fun junk_paints_a_visible_grey_rather_than_throwing_at_draw_time() {
        // A malformed colour must not take the whole grid down mid-frame, and an event drawn in
        // transparent would simply vanish, a worse failure than a grey one.
        val grey = Color(0xFF6B7280)
        assertEquals(grey, parseHexColor(""))
        assertEquals(grey, parseHexColor("#fff"))
        assertEquals(grey, parseHexColor("#zzzzzz"))
    }

    @Test
    fun a_calendar_is_looked_up_by_account_and_id_not_by_id_alone() {
        // A provider key is only unique WITHIN an account, so two accounts can both mint a calendar
        // called "work". Matching on the id alone would paint one account's events in the other's
        // colour, and both would look perfectly plausible.
        val calendars = listOf(
            row("acct-1", "work", "Work", "#2f6fa8"),
            row("acct-2", "work", "Freelance", "#3f8f55"),
        )
        assertEquals("Work", calendars.rowFor("acct-1", "work")?.name)
        assertEquals("Freelance", calendars.rowFor("acct-2", "work")?.name)
        assertNotEquals(
            calendars.rowFor("acct-1", "work")?.color?.hex,
            calendars.rowFor("acct-2", "work")?.color?.hex,
        )
        assertEquals(null, calendars.rowFor("acct-3", "work"))
    }

    @Test
    fun the_theme_picks_the_swatch_and_a_missing_calendar_still_draws() {
        val work = row("acct-1", "work", "Work", "#2f6fa8")
        assertEquals("#2f6fa8", work.color.swatch(dark = false).background)
        assertEquals("#111111", work.color.swatch(dark = true).background)

        // A block naming a calendar the page didn't list should not be invisible.
        val missing: CalendarRow? = null
        assertEquals("#6b7280", missing.swatchOrFallback(dark = false).background)
    }
}
