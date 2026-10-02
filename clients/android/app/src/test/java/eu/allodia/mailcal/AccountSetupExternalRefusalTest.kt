package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.RejectedCertificate

// A refused certificate reaches the panel even though the connect that was refused ran somewhere
// else. `MainActivity.addAccount` connects on a thread of its own and posts its answer back, so
// the refusal cannot be the return value of `onConnect`; it arrives as `externalFailure`. Nothing
// here is about the form object, which has always handled a refusal it was handed: this is about
// it being handed one at all (docs/certificate-exceptions.md rule 3).
@RunWith(RobolectricTestRunner::class)
class AccountSetupExternalRefusalTest {
    @get:Rule val compose = createComposeRule()

    private fun refused() = RejectedCertificate(
        serverName = "127.0.0.1",
        sha256 = "70:C7:C4:7E",
        subjectCommonName = "127.0.0.1",
        subjectOrganisation = "Proton AG",
        issuerCommonName = "127.0.0.1",
        issuerOrganisation = "Proton AG",
        notBefore = 0L,
        notAfter = 1L,
    )

    private fun show(failure: ConnectFailure?) {
        compose.setContent {
            AccountSetupScreen(
                externalError = null,
                externalFailure = failure,
                onCancel = {},
                onSignInMicrosoft = {},
                onConnect = { null },
                onConnectJmap = { null },
            )
        }
    }

    @Test
    fun aRefusalFromTheConnectThreadReachesTheCertificatePanel() {
        show(ConnectFailure("certificate not verified", refused()))
        val ctx = androidx.test.core.app.ApplicationProvider.getApplicationContext<android.content.Context>()

        // What the certificate claims, and the one thing that can be compared against the server.
        compose.onNodeWithText(L10n.setup_certificate_fingerprint(ctx)).assertExists()
        compose.onNodeWithText("70:C7:C4:7E").assertExists()
        // And the acceptance itself, which is the whole point: without it there is nothing to tick.
        compose.onNodeWithText(L10n.setup_certificate_confirm(ctx)).assertExists()
    }

    // The panel says the refusal in the reader's language with the certificate beside it, so the
    // transport's own line is not shown as well (rule 7).
    @Test
    fun theRawTransportMessageIsNotShownBesideThePanel() {
        show(ConnectFailure("certificate not verified: CaUsedAsEndEntity", refused()))
        compose.onNodeWithText("certificate not verified: CaUsedAsEndEntity").assertDoesNotExist()
    }

    // A failure carrying no certificate is still a message, because nothing else would say it.
    @Test
    fun aFailureWithNoCertificateKeepsItsMessage() {
        show(ConnectFailure("Connection refused (os error 111)"))
        compose.onNodeWithText("Connection refused (os error 111)").assertExists()
    }
}
