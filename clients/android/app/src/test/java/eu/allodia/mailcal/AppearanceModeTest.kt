// The MAILCAL_APPEARANCE launch override's spellings, the lever a showcase or UI run pulls to
// photograph both themes without touching the device's own setting. A value it silently ignores
// looks exactly like a working one in the resulting screenshot, so the rule is pinned rather than
// trusted. The spellings are a cross-client contract: scripts/dev/* pass the same three words to
// every platform.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.mailcal_bindings.Appearance

class AppearanceModeTest {
    @Test
    fun theContractSpellingsAreMatchedLiterally() {
        assertEquals(Appearance.LIGHT, AppearanceMode.parse("light"))
        assertEquals(Appearance.DARK, AppearanceMode.parse("dark"))
        // Trimmed and case-insensitive, like every other launch hook.
        assertEquals(Appearance.DARK, AppearanceMode.parse(" DARK "))
        // "system" is an override in its own right, not an absent one: it pins a run to the
        // device's setting even for a developer whose stored choice is Light or Dark.
        assertEquals(Appearance.SYSTEM, AppearanceMode.parse("system"))
    }

    @Test
    fun anythingElseLeavesTheStoredChoiceStanding() {
        assertNull(AppearanceMode.parse(null))
        assertNull(AppearanceMode.parse(""))
        assertNull(AppearanceMode.parse("  "))
        assertNull(AppearanceMode.parse("night"))
        assertNull(AppearanceMode.parse("1"))
    }
}
