// JVM tests for the detection connect-form gating, especially the untrusted-settings
// approval, which is a cross-platform security contract. No Compose or device needed.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.DetectedServerRow
import uniffi.mailcal_bindings.SetupRecommendation

class AccountSetupDetectTest {
    private fun jmap(isTrusted: Boolean) = SetupRecommendation.Jmap(
        email = "alice@example.com",
        serverUrl = "https://example.com",
        isTrusted = isTrusted,
        source = "https://example.com/.well-known/jmap",
    )

    private fun imap(
        isTrusted: Boolean,
        withSmtp: Boolean = true,
        caldavUrl: String? = null,
        security: ConnectionSecurity = ConnectionSecurity.IMPLICIT_TLS,
    ) = SetupRecommendation.Imap(
        email = "alice@example.com",
        imapHost = "imap.example.com",
        smtpHost = if (withSmtp) "smtp.example.com" else null,
        imapSecurity = security,
        smtpSecurity = security,
        incoming = DetectedServerRow("IMAP", "imap.example.com", 993u, "SSL/TLS", "alice@example.com"),
        outgoing = if (withSmtp) DetectedServerRow("SMTP", "smtp.example.com", 465u, "SSL/TLS", "alice@example.com") else null,
        caldavUrl = caldavUrl,
        oauthIssuer = null,
        isTrusted = isTrusted,
        source = "https://autoconfig.example.com/mail/config-v1.1.xml",
    )

    @Test
    fun jmapConnectsWithASecret() {
        val form = DetectedConnectForm(jmap(isTrusted = true))
        assertFalse(form.canConnect)
        form.password = "secret"
        assertTrue(form.canConnect)
        val setup = form.jmapSetup()
        assertEquals("alice@example.com", setup.email)
        assertEquals("https://example.com", setup.serverUrl)
        assertEquals("secret", setup.password)
    }

    @Test
    fun jmapSendsAnApiTokenInTheSameSecretField() {
        // There is no separate token field on the detected card either: an API token rides in
        // `password` so the engine can present it as Basic or Bearer, whichever the server asks.
        val form = DetectedConnectForm(jmap(isTrusted = true))
        form.password = "api-token"
        assertTrue(form.canConnect)
        assertEquals("api-token", form.jmapSetup().password)
    }

    @Test
    fun untrustedJmapRequiresApproval() {
        val form = DetectedConnectForm(jmap(isTrusted = false))
        form.password = "secret"
        assertTrue(form.needsApproval)
        assertFalse("must not connect untrusted settings without approval", form.canConnect)
        form.approved = true
        assertTrue(form.canConnect)
    }

    @Test
    fun imapConnectsWithAPasswordAndAssemblesTheSetup() {
        val form = DetectedConnectForm(imap(isTrusted = true))
        assertFalse(form.needsApproval)
        assertFalse(form.canConnect)
        form.password = "hunter2"
        assertTrue(form.canConnect)
        val setup = form.imapSetup()
        assertEquals("imap.example.com", setup.imapHost)
        assertEquals("alice@example.com", setup.username)
        assertEquals("smtp.example.com", setup.smtpHost)
        assertNull(setup.caldavBaseUrl)
    }

    @Test
    fun untrustedImapRequiresApproval() {
        val form = DetectedConnectForm(imap(isTrusted = false))
        form.password = "hunter2"
        assertFalse(form.canConnect)
        form.approved = true
        assertTrue(form.canConnect)
    }

    @Test
    fun detectedSecurityIsCarriedIntoTheImapSetup() {
        // Implicit-TLS detection passes implicit TLS through; STARTTLS detection passes
        // STARTTLS, so the engine dials exactly what was found.
        val implicit = DetectedConnectForm(imap(isTrusted = true)).also { it.password = "p" }.imapSetup()
        assertEquals(ConnectionSecurity.IMPLICIT_TLS, implicit.imapSecurity)
        assertEquals(ConnectionSecurity.IMPLICIT_TLS, implicit.smtpSecurity)

        val starttls = DetectedConnectForm(imap(isTrusted = true, security = ConnectionSecurity.START_TLS))
            .also { it.password = "p" }.imapSetup()
        assertEquals(ConnectionSecurity.START_TLS, starttls.imapSecurity)
        assertEquals(ConnectionSecurity.START_TLS, starttls.smtpSecurity)
    }

    @Test
    fun imapWithoutSmtpLeavesSmtpNull() {
        val form = DetectedConnectForm(imap(isTrusted = true, withSmtp = false))
        form.password = "hunter2"
        assertNull(form.imapSetup().smtpHost)
    }

    @Test
    fun aDiscoveredCalendarIsPreselectedAndRidesTheImapCredentials() {
        val form = DetectedConnectForm(imap(isTrusted = true, caldavUrl = "https://caldav.soverin.net/calendars"))
        // A found endpoint is opt-out: pre-checked, and stored on connect reusing the password.
        assertTrue(form.calendarEnabled)
        form.password = "hunter2"
        assertEquals("https://caldav.soverin.net/calendars", form.imapSetup().caldavBaseUrl)
    }

    @Test
    fun optingOutOfADiscoveredCalendarLeavesItUnset() {
        val form = DetectedConnectForm(imap(isTrusted = true, caldavUrl = "https://caldav.soverin.net/calendars"))
        form.password = "hunter2"
        form.calendarEnabled = false
        assertNull(form.imapSetup().caldavBaseUrl)
    }

    @Test
    fun withoutADiscoveredCalendarTheUserCanAddOneManually() {
        val form = DetectedConnectForm(imap(isTrusted = true, caldavUrl = null))
        // Nothing discovered → opt-in: off by default until the user turns it on and types a URL.
        assertFalse(form.calendarEnabled)
        form.password = "hunter2"
        assertNull(form.imapSetup().caldavBaseUrl)
        form.calendarEnabled = true
        form.calendarUrlEntry = "caldav.example.com"
        assertEquals("caldav.example.com", form.imapSetup().caldavBaseUrl)
    }
}
