// The documentation screenshot driver (docs/user-docs.md): a showcase run photographs the
// account-setup walkthrough by handing AccountSetupFlow a seed, and the flow types the address and
// runs the lookup by itself.
//
// Worth a test because the failure is *silent and photogenic*: a seed that never fires, or one that
// fires on the wrong step, leaves the flow on the email question, a perfectly clean frame that the
// launch interlock, the blank-frame floor and the manifest hash all accept, filed under
// `setup-detected`. The only thing that can tell them apart is which step is on screen.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.DetectedServerRow
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation

@RunWith(RobolectricTestRunner::class)
class AccountSetupShowcaseSeedTest {
    @get:Rule val compose = createComposeRule()

    // Stands in for the core's scripted detection, keyed on the same domains it is (see
    // SHOWCASE_TRUSTED_DOMAIN / SHOWCASE_UNTRUSTED_DOMAIN in crates/mailcal-bindings). The real one
    // is covered by the Rust suite; what is under test here is that the flow *consults* it.
    private fun scriptedDetect(email: String): SetupRecommendation {
        val domain = email.substringAfterLast('@').lowercase()
        val trusted = domain == "northwind.example"
        if (!trusted && domain != "oldschool.example") {
            return SetupRecommendation.Manual(MissReason.NOTHING_FOUND)
        }
        return SetupRecommendation.Imap(
            email = email,
            imapHost = "imap.$domain",
            smtpHost = "smtp.$domain",
            imapSecurity = ConnectionSecurity.IMPLICIT_TLS,
            smtpSecurity = ConnectionSecurity.IMPLICIT_TLS,
            incoming = DetectedServerRow("IMAP", "imap.$domain", 993u, "SSL/TLS", email),
            outgoing = DetectedServerRow("SMTP", "smtp.$domain", 465u, "SSL/TLS", email),
            caldavUrl = if (trusted) "https://dav.$domain/" else null,
            isTrusted = trusted,
            source = "autoconfig",
        )
    }

    private fun drive(screen: ShowcaseScreen) {
        compose.setContent {
            AccountSetupFlow(
                externalError = null,
                onCancel = null,
                signingIn = false,
                connecting = false,
                detect = ::scriptedDetect,
                onSignInMicrosoft = {},
                onConnect = { null },
                onConnectJmap = { null },
                showcaseSeed = showcaseSetupSeed(screen),
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun theEmailScreenStopsWithTheAddressTyped() {
        drive(ShowcaseScreen.SETUP_EMAIL)
        // Still the question, not an answer: the guide's first image is the field itself.
        compose.onNodeWithText("eva@northwind.example").assertIsDisplayed()
        compose.onNodeWithText("Continue").assertIsDisplayed()
    }

    @Test
    fun theDetectedScreenShowsTheFoundServersAndTheCalendarOptOut() {
        drive(ShowcaseScreen.SETUP_DETECTED)
        compose.onNodeWithText("imap.northwind.example", substring = true).assertIsDisplayed()
        compose.onNodeWithText("smtp.northwind.example", substring = true).assertIsDisplayed()
        // The pre-checked calendar sync only appears when an endpoint was discovered, which is the
        // difference between this capture and the untrusted one below.
        compose.onNodeWithText("dav.northwind.example").assertIsDisplayed()
    }

    @Test
    fun theUntrustedScreenReallyShowsTheApprovalGate() {
        drive(ShowcaseScreen.SETUP_UNTRUSTED)
        // The one assertion here that is a security contract rather than a layout check: the
        // screenshot must picture a warning the app genuinely raises (docs/account-autodetect.md).
        compose.onNodeWithText("I trust these settings").assertIsDisplayed()
        compose.onNodeWithText("imap.oldschool.example", substring = true).assertIsDisplayed()
    }

    @Test
    fun theManualScreenFallsThroughToTheFormWithItsReason() {
        drive(ShowcaseScreen.SETUP_MANUAL)
        // The manual form's own field, which the found card has no equivalent of...
        compose.onNodeWithText("Mail server").assertIsDisplayed()
        // ...and the line saying why the user is looking at it. A capture of the form without it
        // would document a dead end rather than a fallback.
        compose.onNodeWithText("couldn't find settings", substring = true).assertIsDisplayed()
    }

    @Test
    fun noSeedLeavesTheFieldEmptyAndRunsNothing() {
        var lookups = 0
        compose.setContent {
            AccountSetupFlow(
                externalError = null,
                onCancel = null,
                signingIn = false,
                connecting = false,
                detect = { lookups++; scriptedDetect(it) },
                onSignInMicrosoft = {},
                onConnect = { null },
                onConnectJmap = { null },
                showcaseSeed = null,
            )
        }
        compose.waitForIdle()
        // A real launch: nothing typed, nothing looked up. The driver is inert unless asked.
        assert(lookups == 0) { "the setup flow ran $lookups lookup(s) with no showcase seed" }
        compose.onNodeWithText("Continue").assertIsDisplayed()
    }
}
