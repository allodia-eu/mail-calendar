// The showcase (screenshot) screen parser, tested without a UI. `scripts/dev/showcase.sh` passes
// the same MAILCAL_SHOWCASE_SCREEN spellings to every client, so these flag strings are a
// cross-client contract, a typo here (or a client that quietly accepted "addaccount") would
// mislabel a store screenshot. Plain JUnit: ShowcaseScreen.from is a pure function.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ShowcaseScreenTest {
    @Test
    fun everyContractFlagResolves() {
        assertEquals(ShowcaseScreen.LIST, ShowcaseScreen.from("list"))
        assertEquals(ShowcaseScreen.REPLY, ShowcaseScreen.from("reply"))
        assertEquals(ShowcaseScreen.SETTINGS, ShowcaseScreen.from("settings"))
        assertEquals(ShowcaseScreen.ADD_ACCOUNT, ShowcaseScreen.from("add-account"))
        assertEquals(ShowcaseScreen.CALENDAR, ShowcaseScreen.from("calendar"))
        assertEquals(ShowcaseScreen.INVITATION, ShowcaseScreen.from("invitation"))
    }

    @Test
    fun everyDocumentationFlagResolves() {
        // The spellings scripts/dev/showcase.sh puts in DOC_SCREENS_SHARED. An unknown flag falls
        // back to LIST rather than failing, so a typo here would file a photo of the *inbox* under
        // a setup-guide screenshot id and pass every other check in the pipeline.
        assertEquals(ShowcaseScreen.SETUP_EMAIL, ShowcaseScreen.from("setup-email"))
        assertEquals(ShowcaseScreen.SETUP_DETECTED, ShowcaseScreen.from("setup-detected"))
        assertEquals(ShowcaseScreen.SETUP_UNTRUSTED, ShowcaseScreen.from("setup-untrusted"))
        assertEquals(ShowcaseScreen.SETUP_MANUAL, ShowcaseScreen.from("setup-manual"))
    }

    @Test
    fun calendarFlagIsTrimmedAndCaseInsensitive() {
        assertEquals(ShowcaseScreen.CALENDAR, ShowcaseScreen.from("  Calendar "))
    }

    @Test
    fun flagSpellingsMatchTheCrossClientContract() {
        // The exact strings showcase.sh (and the Apple/Windows clients) use, matched literally, not
        // by name, so renaming an enum can't silently break the screenshot pipeline.
        assertEquals("list", ShowcaseScreen.LIST.flag)
        assertEquals("reply", ShowcaseScreen.REPLY.flag)
        assertEquals("settings", ShowcaseScreen.SETTINGS.flag)
        assertEquals("add-account", ShowcaseScreen.ADD_ACCOUNT.flag)
        assertEquals("calendar", ShowcaseScreen.CALENDAR.flag)
        assertEquals("invitation", ShowcaseScreen.INVITATION.flag)
        assertEquals("setup-email", ShowcaseScreen.SETUP_EMAIL.flag)
        assertEquals("setup-detected", ShowcaseScreen.SETUP_DETECTED.flag)
        assertEquals("setup-untrusted", ShowcaseScreen.SETUP_UNTRUSTED.flag)
        assertEquals("setup-manual", ShowcaseScreen.SETUP_MANUAL.flag)
    }

    @Test
    fun onlyTheWalkthroughScreensCarryASetupSeed() {
        // A seed on any other screen would prefill the address field of a real add-account run.
        for (screen in ShowcaseScreen.entries - SETUP_SCREENS) {
            assertNull(screen.flag, showcaseSetupSeed(screen))
        }
    }

    @Test
    fun theSeedAddressesMatchTheCoresDetectionScript() {
        // These domains are the contract with crates/mailcal-bindings/src/autodetect.rs: the core
        // answers *only* for them, so a changed address here silently turns the trusted and
        // untrusted captures into two copies of the manual form.
        assertEquals("eva@northwind.example", showcaseSetupSeed(ShowcaseScreen.SETUP_EMAIL)?.email)
        assertEquals("eva@northwind.example", showcaseSetupSeed(ShowcaseScreen.SETUP_DETECTED)?.email)
        assertEquals("bram@oldschool.example", showcaseSetupSeed(ShowcaseScreen.SETUP_UNTRUSTED)?.email)
        assertEquals("eva.jansen@example.com", showcaseSetupSeed(ShowcaseScreen.SETUP_MANUAL)?.email)
    }

    @Test
    fun onlyTheEmailStepStopsBeforeDetection() {
        // setup-email pictures the field with an address typed and nothing looked up yet; the
        // other three are all *results*, so each must actually run the lookup.
        assertEquals(false, showcaseSetupSeed(ShowcaseScreen.SETUP_EMAIL)?.runDetection)
        for (screen in SETUP_SCREENS - ShowcaseScreen.SETUP_EMAIL) {
            assertTrue(screen.flag, showcaseSetupSeed(screen)?.runDetection == true)
        }
    }

    private companion object {
        val SETUP_SCREENS = setOf(
            ShowcaseScreen.SETUP_EMAIL,
            ShowcaseScreen.SETUP_DETECTED,
            ShowcaseScreen.SETUP_UNTRUSTED,
            ShowcaseScreen.SETUP_MANUAL,
        )
    }

    @Test
    fun unknownOrMissingFlagsFallBackToList() {
        assertEquals(ShowcaseScreen.LIST, ShowcaseScreen.from(null))
        assertEquals(ShowcaseScreen.LIST, ShowcaseScreen.from(""))
        assertEquals(ShowcaseScreen.LIST, ShowcaseScreen.from("nonsense"))
        // Not a spelling we accept, the contract is "add-account", hyphenated.
        assertEquals(ShowcaseScreen.LIST, ShowcaseScreen.from("addaccount"))
    }
}
