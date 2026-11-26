// Setup's sign-in with Nextcloud (docs/nextcloud-sign-in.md rules 2 and 6): the manual form asks
// once per server, a press supersedes the attempt in the air, and a grant that became no account
// is kept for its server until setup is left.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.NextcloudAddressStart
import uniffi.mailcal_bindings.NextcloudSignInStart
import uniffi.mailcal_bindings.SetupRecommendation

private const val CLOUD = "https://cloud.example.org/remote.php/dav"

class NextcloudSetupTest {
    @Test
    fun theManualFormAsksOncePerServerAndOffersOnlyWhereItAnswered() {
        val state = SetupNextcloud()
        assertFalse("nothing typed, nothing asked", state.startAsking(" "))
        assertTrue(state.startAsking("$CLOUD "))
        assertFalse("asked already", state.startAsking(CLOUD))
        assertFalse(state.offers(CLOUD))
        state.answered(CLOUD, true)
        assertTrue(state.offers(" $CLOUD"))
        assertFalse(state.offers("https://elsewhere.example.org/dav"))
    }

    @Test
    fun aPressSupersedesTheAttemptInTheAirAndTheAppComingBackStopsTheSpinner() {
        val state = SetupNextcloud()
        val first = state.begin()
        val firstFlight = NextcloudFlight()
        state.flying(first, firstFlight)
        val second = state.begin()
        assertTrue("the first flight was stopped", firstFlight.isStopped)
        assertFalse(state.ended(first))
        assertTrue(state.signingIn)
        val late = NextcloudFlight()
        state.flying(first, late)
        assertTrue("a flight for a replaced attempt is stopped", late.isStopped)
        state.resumed()
        assertFalse(state.signingIn)
        assertTrue("the attempt still takes its grant", state.ended(second))
    }

    @Test
    fun aKeptGrantIsForItsServerAndLeavingSetupGivesItUp() {
        val state = SetupNextcloud()
        state.hold(CLOUD, "granted")
        assertNull(state.heldFor("https://elsewhere.example.org/dav"))
        assertEquals("granted", state.heldFor(CLOUD))
        val attempt = state.begin()
        val flight = NextcloudFlight()
        state.flying(attempt, flight)
        assertEquals("granted", state.leave())
        assertTrue(flight.isStopped)
        assertFalse(state.signingIn)
        assertNull(state.take())
    }

    @Test
    fun detectionsAnswerReachesTheCardAndTheLinkStep() {
        val setup = detected(SetupRecommendation.Manual(uniffi.mailcal_bindings.MissReason.NOTHING_FOUND))
            .copy(calendarAndContacts = true, caldavUrl = CLOUD, nextcloudSignIn = true)
        val phase = route(setup, "alice@cloud.example.org", signInOffered = false)
        assertTrue(phase is DetectPhase.Dav && phase.nextcloud && !phase.beside)
        val besidePhase = route(setup.copy(besideAccount = true), "alice@example.org", signInOffered = false)
        assertTrue(besidePhase is DetectPhase.Dav && besidePhase.beside)

        val jmap = SetupRecommendation.Jmap("alice@example.org", "https://jmap.example.org", true, "https://example.org")
        val found = FoundBeside.of("alice@example.org", detected(jmap).copy(caldavUrl = CLOUD, nextcloudSignIn = true))
        assertTrue(found?.offerFor("alice@jmap")?.nextcloud == true)
    }

    @Test
    fun thePanesCheckLastsUntilTheBrowserOpensAndUnsureFollowsTheField() {
        val state = SetupNextcloud()
        val attempt = state.begin(checking = true)
        assertTrue(state.checking)
        state.opened(attempt)
        assertFalse("the browser holds it now", state.checking)
        val unsureAttempt = state.begin(checking = true)
        state.ended(unsureAttempt)
        assertFalse(state.checking)
        state.unsure(unsureAttempt, " cloud.example.org ", "cloud.example.org")
        assertEquals("cloud.example.org", state.unsureFor("cloud.example.org "))
        assertNull("another address is not offered", state.unsureFor("other.example.org"))
        state.begin()
        assertNull("trying it answers the offer", state.unsureFor("cloud.example.org"))
    }

    @Test
    fun anUnsureAnswerForAReplacedAttemptOffersNothingAndLeavingClearsIt() {
        val state = SetupNextcloud()
        val stale = state.begin(checking = true)
        state.begin()
        state.unsure(stale, "cloud.example.org", "cloud.example.org")
        assertNull(state.unsureFor("cloud.example.org"))
        val current = state.begin(checking = true)
        state.unsure(current, "cloud.example.org", "cloud.example.org")
        state.leave()
        assertNull(state.unsureFor("cloud.example.org"))
        assertFalse(state.checking)
    }

    @Test
    fun onlyAStartedAnswerFliesAndEveryOtherIsThrownAsItsRefusal() {
        val refused = runCatching { nextcloudAddressStarted(NextcloudAddressStart.Unsure("cloud.example.org")) }
            .exceptionOrNull()
        assertEquals(NextcloudAddressStart.Unsure("cloud.example.org"), (refused as NextcloudAddressRefused).answer)
        val start = NextcloudSignInStart(loginUrl = "https://cloud.example.org/login/v2/flow", pending = "p")
        assertEquals(start, nextcloudAddressStarted(NextcloudAddressStart.Started(start)))
        assertEquals(listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS), NEXTCLOUD_USES)
    }

    @Test
    fun theUsesTheManualFormSignsInForAreTheServersTyped() {
        assertEquals(
            listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS),
            SetupUses.davUses(CLOUD, ""),
        )
    }
}
