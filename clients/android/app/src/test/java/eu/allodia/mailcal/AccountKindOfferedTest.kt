package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.OAuthRoutes

// Which account types the setup picker shows. A browser sign-in needs the provider's OAuth client
// registration, which is injected into the core at build time (BUILDING.md), so this is a property
// of the binary rather than of the address being added, and a build given none must drop the route
// instead of showing a button that fails at the provider.
//
// The decision is a pure function of what the core reported precisely so it can be asserted here:
// the JVM suite never loads the cdylib, so a form that asked the core itself could not be rendered
// in a test at all.
class AccountKindOfferedTest {
    private fun routes(google: Boolean, microsoft: Boolean) = OAuthRoutes(
        google = google,
        googleRedirectUri = null,
        microsoft = microsoft,
    )

    @Test
    fun a_build_with_both_registrations_offers_all_five() {
        assertEquals(
            listOf(
                AccountKind.PASSWORD,
                AccountKind.JMAP,
                AccountKind.DAV,
                AccountKind.MICROSOFT,
                AccountKind.GOOGLE,
            ),
            AccountKind.offered(routes(google = true, microsoft = true)),
        )
    }

    @Test
    fun a_build_with_neither_still_offers_the_three_credential_routes() {
        assertEquals(
            listOf(AccountKind.PASSWORD, AccountKind.JMAP, AccountKind.DAV),
            AccountKind.offered(routes(google = false, microsoft = false)),
        )
    }

    @Test
    fun each_registration_drops_only_its_own_route() {
        assertEquals(
            listOf(AccountKind.PASSWORD, AccountKind.JMAP, AccountKind.DAV, AccountKind.MICROSOFT),
            AccountKind.offered(routes(google = false, microsoft = true)),
        )
        assertEquals(
            listOf(AccountKind.PASSWORD, AccountKind.JMAP, AccountKind.DAV, AccountKind.GOOGLE),
            AccountKind.offered(routes(google = true, microsoft = false)),
        )
    }
}
