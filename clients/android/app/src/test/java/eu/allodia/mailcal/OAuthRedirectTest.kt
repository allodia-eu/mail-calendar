package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// Which sign-in an arriving redirect belongs to. Four flows come back to one activity and two of
// them, JMAP and the Allodia account, ride the same application-id scheme, so only the host tells
// those apart. Routing an Allodia redirect to the JMAP flow does not error: the exchange is handed
// a code minted for a different client and the sign-in the user is waiting on never comes back.
//
// The provider schemes are passed in because they are properties of the injected build, which this
// suite cannot ask the core about.
class OAuthRedirectTest {
    private val app = "eu.allodia.mailcal"
    private val google = "com.googleusercontent.apps.123-abc"
    private val microsoft = "msauth.eu.allodia.mailcal"

    private fun of(scheme: String?, host: String?, googleScheme: String? = google) =
        OAuthRedirect.of(
            scheme = scheme,
            host = host,
            googleScheme = googleScheme,
            microsoftScheme = microsoft,
            appScheme = app,
        )

    @Test
    fun the_two_provider_schemes_route_to_their_own_flows() {
        assertEquals(OAuthRedirect.GOOGLE, of(google, "oauth2redirect"))
        assertEquals(OAuthRedirect.MICROSOFT, of(microsoft, "auth"))
    }

    @Test
    fun our_own_scheme_is_split_by_host_between_jmap_and_the_allodia_account() {
        assertEquals(OAuthRedirect.ALLODIA, of(app, "account-oauth"))
        assertEquals(OAuthRedirect.JMAP, of(app, "jmap-oauth"))
    }

    @Test
    fun a_redirect_we_do_not_recognise_is_nobody_s() {
        assertNull(of("https", "example.com"))
        assertNull(of("mailto", null))
    }

    // A build carrying no Google registration reports a null scheme. A schemeless intent, every
    // notification tap and share arrives as one, must not fall into that arm by matching null.
    @Test
    fun a_schemeless_intent_never_matches_an_absent_registration() {
        assertNull(of(null, null, googleScheme = null))
    }
}
